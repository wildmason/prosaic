use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Fields, GenericArgument, Ident, LitStr, PathArguments, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

/// Derive `IntoContext` for a struct, converting its fields into `Context` key-value pairs.
///
/// Field type mapping:
/// - `String` / `&str` / `&'a str` → `Value::String` (borrowed strs are cloned)
/// - `i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32` → `Value::Number`
///   via infallible `as i64` cast
/// - `u64`, `usize` → `Value::Number` via **saturating** conversion:
///   values above `i64::MAX` (≈9.2 × 10¹⁸) saturate to `i64::MAX` rather
///   than wrapping to a negative. If you need the raw bit pattern, convert
///   explicitly in host code before constructing the context.
/// - `Vec<String>` → `Value::List`
/// - `Option<T>` where `T` is any of the above → inserted only when `Some(_)`
///
/// Unsupported field types produce a compile-time error so template slots
/// cannot silently disappear.
#[proc_macro_derive(IntoContext)]
pub fn derive_into_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input.ident,
                    "IntoContext can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "IntoContext can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut insertions = Vec::with_capacity(fields.len());
    let mut schema_entries = Vec::with_capacity(fields.len());

    for field in fields {
        let field_name = match &field.ident {
            Some(ident) => ident,
            None => continue,
        };
        let key = field_name.to_string();
        let ty = &field.ty;

        // Value-type mapping (for schema) — unwraps Option<T> to T.
        let effective_ty = extract_option_inner(ty).unwrap_or(ty);
        let value_type_tokens = match value_type_for_rust_type(effective_ty) {
            Some(t) => t,
            None => {
                let was_opt = extract_option_inner(ty).is_some();
                return unsupported_field_error(field_name, effective_ty, was_opt);
            }
        };
        schema_entries.push(quote! { (#key, #value_type_tokens) });

        // IntoContext insertion (unchanged from prior implementation).
        let conversion = if let Some(inner_ty) = extract_option_inner(ty) {
            match value_conversion_for_type(inner_ty, &quote!(val)) {
                Some(conv) => quote! {
                    if let ::core::option::Option::Some(val) = self.#field_name {
                        ctx.insert(#key, #conv);
                    }
                },
                None => return unsupported_field_error(field_name, inner_ty, true),
            }
        } else {
            match value_conversion_for_type(ty, &quote!(self.#field_name)) {
                Some(conv) => quote! {
                    ctx.insert(#key, #conv);
                },
                None => return unsupported_field_error(field_name, ty, false),
            }
        };

        insertions.push(conversion);
    }

    let expanded = quote! {
        impl #impl_generics ::prosaic_core::IntoContext for #name #ty_generics #where_clause {
            fn into_context(self) -> ::prosaic_core::Context {
                let mut ctx = ::prosaic_core::Context::new();
                #(#insertions)*
                ctx
            }
        }

        impl #impl_generics ::prosaic_core::HasProsaicSchema for #name #ty_generics #where_clause {
            const PROSAIC_SCHEMA: &'static [(&'static str, ::prosaic_core::ValueType)] = &[
                #(#schema_entries),*
            ];
        }
    };

    TokenStream::from(expanded)
}

fn unsupported_field_error(field: &syn::Ident, ty: &Type, was_option: bool) -> TokenStream {
    let wrapper = if was_option { "Option<…>" } else { "" };
    let message = format!(
        "IntoContext: field `{field}` has unsupported type {wrapper}`{ty}`. \
         Supported types are String, &str, integer types (i8..i64/isize/u8..u32/u64/usize), \
         bool, Vec<String>, and Option<T> wrapping any of the above.",
        field = field,
        wrapper = wrapper,
        ty = quote!(#ty),
    );
    syn::Error::new_spanned(field, message)
        .to_compile_error()
        .into()
}

fn value_conversion_for_type(
    ty: &Type,
    accessor: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    if is_type(ty, "String") {
        Some(quote! { ::prosaic_core::Value::String(#accessor) })
    } else if is_str_reference(ty) {
        // `&str` / `&'a str` — clone into an owned String so it fits Value.
        Some(quote! { ::prosaic_core::Value::String((#accessor).to_string()) })
    } else if is_safe_numeric_type(ty) {
        Some(quote! { ::prosaic_core::Value::Number(#accessor as i64) })
    } else if is_wide_numeric_type(ty) {
        // u64 / usize may exceed i64::MAX on 64-bit platforms — saturate
        // to i64::MAX rather than silently wrapping to a negative number.
        Some(quote! {
            ::prosaic_core::Value::Number(
                ::core::convert::TryFrom::try_from(#accessor)
                    .unwrap_or(::core::primitive::i64::MAX)
            )
        })
    } else if is_type(ty, "bool") {
        // Match IntoValue for bool: true → 1, false → 0.
        Some(quote! {
            ::prosaic_core::Value::Number(if #accessor { 1_i64 } else { 0_i64 })
        })
    } else if is_vec_string(ty) {
        Some(quote! { ::prosaic_core::Value::List(#accessor) })
    } else {
        None
    }
}

fn is_type(ty: &Type, name: &str) -> bool {
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| seg.ident == name)
    } else {
        false
    }
}

/// Numeric types whose full range fits in `i64` via an infallible `as` cast.
///
/// `isize` is included because on 32-bit targets it's `i32` (fits trivially)
/// and on 64-bit it equals `i64` (same range). `usize` and `u64` go through
/// the wide path because on 64-bit they may exceed `i64::MAX`.
fn is_safe_numeric_type(ty: &Type) -> bool {
    let safe = ["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32"];
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| safe.contains(&seg.ident.to_string().as_str()))
    } else {
        false
    }
}

/// Numeric types that require a saturating `TryFrom<_, i64>` conversion
/// because they can exceed `i64::MAX` on 64-bit platforms.
fn is_wide_numeric_type(ty: &Type) -> bool {
    let wide = ["u64", "usize"];
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| wide.contains(&seg.ident.to_string().as_str()))
    } else {
        false
    }
}

fn is_vec_string(ty: &Type) -> bool {
    if let Type::Path(type_path) = ty
        && let Some(seg) = type_path.path.segments.last()
        && seg.ident == "Vec"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return is_type(inner, "String");
    }
    false
}

fn is_str_reference(ty: &Type) -> bool {
    if let Type::Reference(r) = ty {
        return is_type(&r.elem, "str");
    }
    false
}

fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty
        && let Some(seg) = type_path.path.segments.last()
        && seg.ident == "Option"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

/// Map a Rust field type to the `ValueType` it projects into when inserted
/// into a `Context`. Returns `None` if the type is unsupported (caller
/// raises `unsupported_field_error`).
fn value_type_for_rust_type(ty: &Type) -> Option<proc_macro2::TokenStream> {
    if is_type(ty, "String") || is_str_reference(ty) {
        Some(quote! { ::prosaic_core::ValueType::String })
    } else if is_safe_numeric_type(ty) || is_wide_numeric_type(ty) || is_type(ty, "bool") {
        Some(quote! { ::prosaic_core::ValueType::Number })
    } else if is_vec_string(ty) {
        Some(quote! { ::prosaic_core::ValueType::List })
    } else {
        None
    }
}

// ── prosaic_template! ──────────────────────────────────────────────────────────

/// Compile-time-validated template string.
///
/// Parses the template, checks every slot reference against the declared
/// `slots` list, and checks every pipe name against the engine's known-pipe
/// set. On success, expands to the original template string literal (`&'static str`).
/// On mismatch, emits a compile error pointing at the `template:` argument.
///
/// When `context: <Type>` is provided, the macro also emits `const` assertions
/// at compile time, verifying that each slot's required type (inferred from its
/// pipe chain) is compatible with the corresponding field in `<Type>`'s
/// `HasProsaicSchema` implementation. A missing slot or type mismatch is a
/// hard compile error with a clear message identifying the slot and context type.
///
/// # Syntax
///
/// ```
/// use prosaic_derive::prosaic_template;
///
/// let tpl: &'static str = prosaic_template! {
///     template: "The {entity_type} {name|refer} was renamed to {new_name}",
///     slots: [entity_type, name, new_name],
/// };
/// assert!(tpl.contains("{name|refer}"));
/// ```
///
/// The `slots:` list uses bare identifiers matching the slot keys in the
/// template. Declaring extra slots that are not used in the template is
/// allowed. Slots used by conditional guards (`{?key}`) must also be declared.
///
/// # Limitations (v1)
///
/// - Pipe *arguments* (`truncate:3`, `verb:past`, etc.) are not validated — only the pipe name.
/// - Slots inside partial inclusions (`{>name}`) are not validated — partials
///   are opaque at compile time and resolved by the engine at registration time.
/// - Compile-fail tests require an external `trybuild` harness (deferred to v2).
#[proc_macro]
pub fn prosaic_template(input: TokenStream) -> TokenStream {
    let parsed = parse_macro_input!(input as ProsaicTemplateInput);

    match validate_template(&parsed) {
        Ok(assertions) => {
            let lit = &parsed.template;
            quote! { { #assertions #lit } }.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

struct ProsaicTemplateInput {
    template: LitStr,
    slots: Vec<Ident>,
    context: Option<syn::Path>,
}

impl Parse for ProsaicTemplateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut template: Option<LitStr> = None;
        let mut slots: Option<Vec<Ident>> = None;
        let mut context: Option<syn::Path> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            match key.to_string().as_str() {
                "template" => {
                    template = Some(input.parse::<LitStr>()?);
                }
                "slots" => {
                    let content;
                    syn::bracketed!(content in input);
                    let parsed_idents: Punctuated<Ident, Token![,]> =
                        Punctuated::parse_terminated(&content)?;
                    slots = Some(parsed_idents.into_iter().collect());
                }
                "context" => {
                    context = Some(input.parse::<syn::Path>()?);
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!(
                            "unknown key `{other}` — expected `template`, `slots`, or `context`"
                        ),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let template = template
            .ok_or_else(|| syn::Error::new(input.span(), "missing `template: \"...\"` argument"))?;
        let slots = slots.unwrap_or_default();

        Ok(ProsaicTemplateInput {
            template,
            slots,
            context,
        })
    }
}

fn validate_template(input: &ProsaicTemplateInput) -> syn::Result<proc_macro2::TokenStream> {
    let template_str = input.template.value();
    let span = input.template.span();

    let parsed = prosaic_core::Template::parse(&template_str)
        .map_err(|e| syn::Error::new(span, format!("invalid template: {e}")))?;

    let declared: std::collections::HashSet<String> =
        input.slots.iter().map(|i| i.to_string()).collect();

    validate_slots(&parsed, &declared, span)?;
    validate_pipes(&parsed, span)?;

    // Infer per-slot types using the shared PIPE_SPECS registry. Chain
    // mismatches and multi-mention conflicts surface here as compile errors.
    let inferred = parsed
        .infer_types()
        .map_err(|reason| syn::Error::new(span, reason))?;

    let assertions = match &input.context {
        Some(ctx_path) => emit_context_assertions(ctx_path, &inferred),
        None => proc_macro2::TokenStream::new(),
    };

    Ok(assertions)
}

fn emit_context_assertions(
    ctx_path: &syn::Path,
    inferred: &[(String, prosaic_core::ValueType)],
) -> proc_macro2::TokenStream {
    use prosaic_core::ValueType;

    let mut stmts = proc_macro2::TokenStream::new();
    for (slot, expected) in inferred {
        let expected_tok = match expected {
            ValueType::String => quote! { ::prosaic_core::ValueType::String },
            ValueType::Number => quote! { ::prosaic_core::ValueType::Number },
            ValueType::List => quote! { ::prosaic_core::ValueType::List },
            ValueType::Entity => quote! { ::prosaic_core::ValueType::Entity },
            ValueType::Any => {
                // A slot inferred as Any imposes no constraint on the context.
                continue;
            }
        };

        let ctx_name_str = quote!(#ctx_path).to_string();
        let missing_msg = format!(
            "prosaic_template: slot `{slot}` is not declared in context `{ctx_name_str}` (no matching field)"
        );
        let mismatch_msg = format!(
            "prosaic_template: slot `{slot}` in context `{ctx_name_str}` has an incompatible type — required by template pipe chain"
        );

        stmts.extend(quote! {
            const _: () = {
                let actual = match ::prosaic_core::schema_lookup(
                    <#ctx_path as ::prosaic_core::HasProsaicSchema>::PROSAIC_SCHEMA,
                    #slot,
                ) {
                    ::core::option::Option::Some(t) => t,
                    ::core::option::Option::None => ::core::panic!(#missing_msg),
                };
                if !::prosaic_core::types_compatible(actual, #expected_tok) {
                    ::core::panic!(#mismatch_msg);
                }
            };
        });
    }
    stmts
}

fn validate_slots(
    template: &prosaic_core::Template,
    declared: &std::collections::HashSet<String>,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let used = template.slot_keys();
    let mut undeclared: Vec<String> = used.into_iter().filter(|k| !declared.contains(k)).collect();
    undeclared.sort();
    undeclared.dedup();

    if !undeclared.is_empty() {
        let list = undeclared.join(", ");
        let mut declared_sorted: Vec<_> = declared.iter().cloned().collect();
        declared_sorted.sort();
        let declared_list = declared_sorted.join(", ");
        return Err(syn::Error::new(
            span,
            format!(
                "template uses slot(s) not declared in `slots: [...]`: {list}\n  declared: [{declared_list}]",
            ),
        ));
    }
    Ok(())
}

fn validate_pipes(template: &prosaic_core::Template, span: proc_macro2::Span) -> syn::Result<()> {
    let used = template.pipe_names();
    let mut unknown: Vec<String> = used
        .into_iter()
        .filter(|p| {
            !prosaic_core::PIPE_SPECS
                .iter()
                .any(|spec| spec.name == p.as_str())
        })
        .collect();
    unknown.sort();
    unknown.dedup();

    if !unknown.is_empty() {
        let list = unknown
            .iter()
            .map(|p| match nearest_pipe(p) {
                Some(s) => format!("`{p}` (did you mean `{s}`?)"),
                None => format!("`{p}`"),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let known: Vec<&str> = prosaic_core::PIPE_SPECS.iter().map(|s| s.name).collect();
        return Err(syn::Error::new(
            span,
            format!(
                "template uses unknown pipe(s): {list}\n  known pipes: [{}]",
                known.join(", ")
            ),
        ));
    }
    Ok(())
}

fn nearest_pipe(unknown: &str) -> Option<&'static str> {
    let mut names = prosaic_core::PIPE_SPECS.iter().map(|s| s.name);
    // Exact prefix / suffix match first (catches common truncations).
    if let Some(valid) = names
        .clone()
        .find(|&v| v.starts_with(unknown) || unknown.starts_with(v))
    {
        return Some(valid);
    }
    // Fallback: any pipe sharing the first three characters.
    let prefix: String = unknown.chars().take(3).collect();
    names.find(|&v| v.starts_with(prefix.as_str()))
}

// ── prosaic_template_compiled! ─────────────────────────────────────────────────

/// Compile-time compiled template rendering function.
///
/// Parses the template at compile time and emits a specialized render function
/// that avoids the runtime parsing pipeline. Suitable for tight loops with
/// known, simple templates.
///
/// Returns a `fn(&prosaic_core::Context) -> String` as a block expression.
///
/// # Supported syntax
///
/// Only bare slot references are supported: `{key}` and literal text.
/// The following will produce a **compile error**:
/// - Pipes: `{key|capitalize}`
/// - Conditional sections: `{?key}...{/?}`
/// - Partial inclusions: `{>name}`
///
/// For templates requiring any of the above, use the runtime engine directly.
///
/// # Example
///
/// ```
/// use prosaic_derive::prosaic_template_compiled;
/// use prosaic_core::{Context, Value};
///
/// let render = prosaic_template_compiled!("The class {name} was modified");
/// let mut ctx = Context::new();
/// ctx.insert("name", Value::String("Foo".into()));
/// assert_eq!(render(&ctx), "The class Foo was modified");
/// ```
#[proc_macro]
pub fn prosaic_template_compiled(input: TokenStream) -> TokenStream {
    let template_lit = parse_macro_input!(input as LitStr);
    let template_str = template_lit.value();
    let span = template_lit.span();

    // Parse the template using the core runtime parser.
    let parsed = match prosaic_core::Template::parse(&template_str) {
        Ok(t) => t,
        Err(e) => {
            return syn::Error::new(span, format!("invalid template: {e}"))
                .to_compile_error()
                .into();
        }
    };

    // Validate: only bare slots are supported. as_bare_slots() returns None if
    // the template contains pipes, conditionals, or partials.
    let bare_segments = match parsed.as_bare_slots() {
        Some(segs) => segs,
        None => {
            // Give a precise error: detect which unsupported feature is present.
            let has_pipes = !parsed.pipe_names().is_empty();
            let msg = if has_pipes {
                "prosaic_template_compiled!: templates with pipes are not supported; use the runtime engine"
            } else {
                "prosaic_template_compiled!: conditional sections, partials, and advanced features are not supported; use the runtime engine"
            };
            return syn::Error::new(span, msg).to_compile_error().into();
        }
    };

    // Estimate initial capacity as template length (reasonable lower bound).
    let capacity = template_str.len();

    // Generate the push_str calls for each segment.
    let mut stmts = Vec::new();
    for seg in &bare_segments {
        match seg {
            prosaic_core::BareSegment::Text(text) => {
                stmts.push(quote! { out.push_str(#text); });
            }
            prosaic_core::BareSegment::Slot(key) => {
                stmts.push(quote! {
                    if let Some(__v) = __ctx.get(#key) {
                        out.push_str(&__v.as_display());
                    }
                });
            }
        }
    }

    let expanded = quote! {
        {
            fn __prosaic_compiled_render(__ctx: &::prosaic_core::Context) -> ::std::string::String {
                let mut out = ::std::string::String::with_capacity(#capacity);
                #(#stmts)*
                out
            }
            __prosaic_compiled_render
        }
    };

    expanded.into()
}

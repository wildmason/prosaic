use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    Data, DeriveInput, Fields, GenericArgument, Ident, LitStr, PathArguments, Token, Type,
};

/// Derive `IntoContext` for a struct, converting its fields into `Context` key-value pairs.
///
/// Field type mapping:
/// - `String` / `&str` / `&'a str` → `Value::String` (borrowed strs are cloned)
/// - `i8`, `i16`, `i32`, `i64`, `isize`, `u8`, `u16`, `u32`, `u64`, `usize` → `Value::Number`
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
    for field in fields {
        let field_name = match &field.ident {
            Some(ident) => ident,
            None => continue,
        };
        let key = field_name.to_string();
        let ty = &field.ty;

        let conversion = if let Some(inner_ty) = extract_option_inner(ty) {
            match value_conversion_for_type(inner_ty, &quote!(val)) {
                Some(conv) => quote! {
                    if let ::core::option::Option::Some(val) = self.#field_name {
                        ctx.insert(#key, #conv);
                    }
                },
                None => {
                    return unsupported_field_error(field_name, inner_ty, true);
                }
            }
        } else {
            match value_conversion_for_type(ty, &quote!(self.#field_name)) {
                Some(conv) => quote! {
                    ctx.insert(#key, #conv);
                },
                None => {
                    return unsupported_field_error(field_name, ty, false);
                }
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
    };

    TokenStream::from(expanded)
}

fn unsupported_field_error(
    field: &syn::Ident,
    ty: &Type,
    was_option: bool,
) -> TokenStream {
    let wrapper = if was_option { "Option<…>" } else { "" };
    let message = format!(
        "IntoContext: field `{field}` has unsupported type {wrapper}`{ty}`. \
         Supported types are String, &str, integer types, Vec<String>, and \
         Option<T> wrapping any of the above.",
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
    } else if is_numeric_type(ty) {
        Some(quote! { ::prosaic_core::Value::Number(#accessor as i64) })
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

fn is_numeric_type(ty: &Type) -> bool {
    let numeric_types = [
        "i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize",
    ];
    if let Type::Path(type_path) = ty {
        type_path
            .path
            .segments
            .last()
            .is_some_and(|seg| numeric_types.contains(&seg.ident.to_string().as_str()))
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

// ── prosaic_template! ──────────────────────────────────────────────────────────

/// Pipe names that the NLG engine's `apply_pipe` dispatch recognises.
/// Kept in sync with `engine.rs::apply_pipe`. Used by `prosaic_template!` for
/// compile-time pipe validation.
const VALID_PIPES: &[&str] = &[
    "plural",
    "pluralize",
    "article",
    "join",
    "ordinal",
    "words",
    "truncate",
    "capitalize",
    "refer",
    "verb",
    "syn",
    "relative",
    "since_last",
    "quantify",
    "hedge",
    "negated",
    "choose",
];

/// Compile-time-validated template string.
///
/// Parses the template, checks every slot reference against the declared
/// `slots` list, and checks every pipe name against the engine's known-pipe
/// set. On success, expands to the original template string literal (`&'static str`).
/// On mismatch, emits a compile error pointing at the `template:` argument.
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
        Ok(()) => {
            let lit = &parsed.template;
            quote! { #lit }.into()
        }
        Err(e) => e.to_compile_error().into(),
    }
}

struct ProsaicTemplateInput {
    template: LitStr,
    slots: Vec<Ident>,
}

impl Parse for ProsaicTemplateInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut template: Option<LitStr> = None;
        let mut slots: Option<Vec<Ident>> = None;

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
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown key `{other}` — expected `template` or `slots`"),
                    ));
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let template = template.ok_or_else(|| {
            syn::Error::new(input.span(), "missing `template: \"...\"` argument")
        })?;
        let slots = slots.unwrap_or_default();

        Ok(ProsaicTemplateInput { template, slots })
    }
}

fn validate_template(input: &ProsaicTemplateInput) -> syn::Result<()> {
    let template_str = input.template.value();
    let span = input.template.span();

    let parsed = prosaic_core::Template::parse(&template_str).map_err(|e| {
        syn::Error::new(span, format!("invalid template: {e}"))
    })?;

    let declared: std::collections::HashSet<String> =
        input.slots.iter().map(|i| i.to_string()).collect();

    validate_slots(&parsed, &declared, span)?;
    validate_pipes(&parsed, span)?;

    Ok(())
}

fn validate_slots(
    template: &prosaic_core::Template,
    declared: &std::collections::HashSet<String>,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let used = template.slot_keys();
    let mut undeclared: Vec<String> = used
        .into_iter()
        .filter(|k| !declared.contains(k))
        .collect();
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

fn validate_pipes(
    template: &prosaic_core::Template,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let used = template.pipe_names();
    let mut unknown: Vec<String> = used
        .into_iter()
        .filter(|p| !VALID_PIPES.contains(&p.as_str()))
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
        return Err(syn::Error::new(
            span,
            format!(
                "template uses unknown pipe(s): {list}\n  known pipes: [{}]",
                VALID_PIPES.join(", ")
            ),
        ));
    }
    Ok(())
}

fn nearest_pipe(unknown: &str) -> Option<&'static str> {
    // Exact prefix / suffix match first (catches common truncations).
    if let Some(&valid) = VALID_PIPES
        .iter()
        .find(|&&v| v.starts_with(unknown) || unknown.starts_with(v))
    {
        return Some(valid);
    }
    // Fallback: any pipe sharing the first three characters.
    let prefix: String = unknown.chars().take(3).collect();
    VALID_PIPES
        .iter()
        .find(|&&v| v.starts_with(prefix.as_str()))
        .copied()
}

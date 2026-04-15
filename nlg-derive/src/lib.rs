use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, GenericArgument, PathArguments, Type};

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
        impl #impl_generics ::nlg_core::IntoContext for #name #ty_generics #where_clause {
            fn into_context(self) -> ::nlg_core::Context {
                let mut ctx = ::nlg_core::Context::new();
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
        Some(quote! { ::nlg_core::Value::String(#accessor) })
    } else if is_str_reference(ty) {
        // `&str` / `&'a str` — clone into an owned String so it fits Value.
        Some(quote! { ::nlg_core::Value::String((#accessor).to_string()) })
    } else if is_numeric_type(ty) {
        Some(quote! { ::nlg_core::Value::Number(#accessor as i64) })
    } else if is_vec_string(ty) {
        Some(quote! { ::nlg_core::Value::List(#accessor) })
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

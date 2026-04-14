use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields, Type, PathArguments, GenericArgument};

/// Derive `IntoContext` for a struct, converting its fields into `Context` key-value pairs.
///
/// Field type mapping:
/// - `String` / `&str` → `Value::String`
/// - `i64`, `i32`, `i16`, `i8`, `u64`, `u32`, `u16`, `u8`, `usize`, `isize` → `Value::Number`
/// - `Vec<String>` → `Value::List`
///
/// Fields with `Option<T>` are only inserted if `Some`.
#[proc_macro_derive(IntoContext)]
pub fn derive_into_context(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

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

    let insertions: Vec<_> = fields
        .iter()
        .filter_map(|field| {
            let field_name = field.ident.as_ref()?;
            let key = field_name.to_string();
            let ty = &field.ty;

            if let Some(inner_ty) = extract_option_inner(ty) {
                // Option<T> — only insert if Some
                let conversion = value_conversion_for_type(inner_ty, &quote!(val));
                conversion.map(|conv| {
                    quote! {
                        if let Some(val) = self.#field_name {
                            ctx.insert(#key, #conv);
                        }
                    }
                })
            } else {
                let conversion = value_conversion_for_type(ty, &quote!(self.#field_name));
                conversion.map(|conv| {
                    quote! {
                        ctx.insert(#key, #conv);
                    }
                })
            }
        })
        .collect();

    let expanded = quote! {
        impl nlg_core::IntoContext for #name {
            fn into_context(self) -> nlg_core::Context {
                let mut ctx = nlg_core::Context::new();
                #(#insertions)*
                ctx
            }
        }
    };

    TokenStream::from(expanded)
}

fn value_conversion_for_type(
    ty: &Type,
    accessor: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    if is_type(ty, "String") {
        Some(quote! { nlg_core::Value::String(#accessor) })
    } else if is_numeric_type(ty) {
        Some(quote! { nlg_core::Value::Number(#accessor as i64) })
    } else if is_vec_string(ty) {
        Some(quote! { nlg_core::Value::List(#accessor) })
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
        "i8", "i16", "i32", "i64", "i128", "isize",
        "u8", "u16", "u32", "u64", "u128", "usize",
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
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Vec" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return is_type(inner, "String");
                    }
                }
            }
        }
    }
    false
}

fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(type_path) = ty {
        if let Some(seg) = type_path.path.segments.last() {
            if seg.ident == "Option" {
                if let PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

use std::ops::Not;

use darling::ast::Style;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_macro_input, parse_quote, DeriveInput, Generics, Ident, PredicateType, TypePath};

use darling::usage::{CollectTypeParams, GenericsExt, IdentRefSet, Purpose};
use darling::FromDeriveInput;

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(bounded_to))]
struct BoundedDerive {
    ident: syn::Ident,
    generics: syn::Generics,
    data: darling::ast::Data<syn::Variant, syn::Field>,
    types: Punctuated<syn::Path, Comma>,
}

fn root_idents(types: &Punctuated<syn::Path, Comma>) -> IdentRefSet {
    types.iter().map(|path| &path.segments[0].ident).collect()
}

fn normalize_generics<'a>(
    bound: TokenStream2,
    generics: &mut Generics,
    types: impl Iterator<Item = &'a syn::Path>,
) {
    let bounds = generics
        .type_params_mut()
        .filter_map(|par| {
            par.bounds.is_empty().not().then_some({
                let bounds = std::mem::take(&mut par.bounds);

                let path: syn::Path = par.ident.clone().into();
                let path = TypePath { qself: None, path };

                PredicateType {
                    lifetimes: None,
                    bounded_ty: path.into(),
                    colon_token: Default::default(),
                    bounds,
                }
            })
        })
        .collect::<Vec<_>>();

    let where_clause = generics.make_where_clause();
    for bound in bounds {
        where_clause.predicates.push(bound.into());
    }

    for ty in types {
        let pred: syn::WherePredicate = parse_quote! { #ty: #bound };

        where_clause.predicates.push(pred.into());
    }
}

fn common_bounded(
    items: TokenStream,
    body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,
    field: fn(field: &Ident) -> TokenStream2,
    bound: TokenStream2,
) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(items);
    let default = match BoundedDerive::from_derive_input(&input) {
        Ok(val) => val,
        Err(err) => {
            return err.write_errors().into();
        }
    };

    let type_params = default.generics.declared_type_params();
    let mut generics = default.generics.clone();

    let inner = match default.data {
        darling::ast::Data::Struct(ref fields) => {
            let type_params_in_body = fields
                .iter()
                .collect_type_params(&Purpose::BoundImpl.into(), &type_params);

            let leftovers = type_params_in_body
                .difference(&root_idents(&default.types))
                .map(|&ident| syn::Path::from(ident.clone()))
                .collect::<Vec<_>>();

            normalize_generics(
                bound,
                &mut generics,
                default.types.iter().chain(leftovers.iter()),
            );
            match fields.style {
                Style::Struct => {
                    // SAFETY: Struct style struct has always fields
                    TokenStream2::from_iter(
                        fields
                            .fields
                            .iter()
                            .map(|f| field(f.ident.as_ref().unwrap())),
                    )
                }
                _ => todo!(),
            }
        }
        darling::ast::Data::Enum(ref _variants) => {
            todo!()
        }
    };

    body(&default.ident, generics, inner).into()
}

#[proc_macro_derive(Default, attributes(bounded_to))]
pub fn default_bounded(items: TokenStream) -> TokenStream {
    let body = |name: &Ident, generics: Generics, inner| -> TokenStream2 {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        quote! {
            impl #impl_generics std::default::Default for #name #ty_generics #where_clause {
                fn default() -> Self {
                    Self {
                        #inner
                    }
                }
            }
        }
    };

    let field = |field: &Ident| -> TokenStream2 {
        quote! { #field: Default::default(), }
    };

    let bound = quote! { Default };

    common_bounded(items, body, field, bound)
}

#[proc_macro_derive(Clone, attributes(bounded_to))]
pub fn clone_bounded(items: TokenStream) -> TokenStream {
    let body = |name: &Ident, generics: Generics, inner| -> TokenStream2 {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        quote! {
            impl #impl_generics std::clone::Clone for #name #ty_generics #where_clause {
                fn clone(&self) -> Self {
                    Self {
                        #inner
                    }
                }
            }
        }
    };

    let field = |field: &Ident| -> TokenStream2 {
        quote! { #field: self.#field.clone(), }
    };

    let bound = quote! { Clone };

    common_bounded(items, body, field, bound)
}

//! Collection of derive macros with explicitable bounds
//!
//! They derive the matching trait but have an optional `bounded_to` attribute to override the
//! bounds.
//!
//! ```
//! use derive_bounded::Clone;
//!
//! trait Trait {
//!     type B: Clone;
//! }
//!
//! #[derive(Clone)]
//! #[bounded_to(types="T::B")]
//! struct A<T: Trait> {
//!     f: T::B,
//! }
//!
//! #[derive(Clone)]
//! #[bounded_to(types="T::B")]
//! struct B<T: Trait> {
//!     f: A<T>,
//! }
//! ```
//!
//! The auto-generated impl for [Clone][std::clone::Clone] will have a where clause with `T::B: Clone` instead of `T: Clone`.
//!
//! As this version there are few known limitations:
//!
//! - The macro works only with Struct-style Structs
//! - The macro does not auto-generate the where clause for associated traits (e.g. `A` in the
//! example needs the `bounded_to` attribute
//!
//! Later versions will address those.
//!

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

#[derive(std::fmt::Debug, FromDeriveInput)]
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

/// Derive [Default][std::default::Default]
///
/// Use the attribute `#[bounded_to(types = "T, A::B")] to to specify more precise bounds.
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

/// Derive [Clone][std::clone::Clone]
///
/// Use the attribute `#[bounded_to(types = "T, A::B")] to to specify more precise bounds.
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

/// Derive [Debug][std::fmt::Debug]
///
/// Use the attribute `#[bounded_to(types = "T, A::B")] to to specify more precise bounds.
#[proc_macro_derive(Debug, attributes(bounded_to))]
pub fn debug_bounded(items: TokenStream) -> TokenStream {
    let body = |name: &Ident, generics: Generics, inner| -> TokenStream2 {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let s = name.to_string();
        quote! {
            impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    f.debug_struct(#s)
                    #inner
                    .finish()
                }
            }
        }
    };

    let field = |field: &Ident| -> TokenStream2 {
        let s = field.to_string();
        quote! { .field(#s, &self.#field) }
    };

    let bound = quote! { std::fmt::Debug };

    common_bounded(items, body, field, bound)
}

/// Derive [PartialEq][std::cmp::PartialEq]
///
/// Use the attribute `#[bounded_to(types = "T, A::B")] to to specify more precise bounds.
#[proc_macro_derive(PartialEq, attributes(bounded_to))]
pub fn partial_eq_bounded(items: TokenStream) -> TokenStream {
    let body = |name: &Ident, generics: Generics, inner| -> TokenStream2 {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        quote! {
            impl #impl_generics std::cmp::PartialEq for #name #ty_generics #where_clause {
                fn eq(&self, other: &Self) -> bool {
                    true
                    #inner
                }
            }
        }
    };

    let field = |field: &Ident| -> TokenStream2 {
        quote! { && other.#field == self.#field }
    };

    let bound = quote! { PartialEq };

    common_bounded(items, body, field, bound)
}

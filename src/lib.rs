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
//! #[bounded_to(T::B)]
//! struct A<T: Trait> {
//!     f: T::B,
//! }
//!
//! #[derive(Clone)]
//! #[bounded_to(T::B)]
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
use proc_macro2::{TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_macro_input, parse_quote, DeriveInput, Generics, Ident, PredicateType, TypePath};

use darling::usage::{CollectTypeParams, GenericsExt, Purpose};
use darling::FromDeriveInput;

#[derive(std::fmt::Debug, FromDeriveInput)]
#[darling(forward_attrs(bounded_to))]
struct BoundedDerive {
    ident: syn::Ident,
    generics: syn::Generics,
    data: darling::ast::Data<syn::Variant, syn::Field>,
    attrs: Vec<syn::Attribute>,
    //types: BoundedTypes,
}

fn normalize_generics<'a>(
    bound: TokenStream2,
    generics: &mut Generics,
    types: impl Iterator<Item = &'a syn::Type>,
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

struct Generator {
    named_body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,
    unnamed_body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,

    named_field: fn(field: &Ident) -> TokenStream2,
    unnamed_field: fn(index: syn::Index) -> TokenStream2,
}

fn common_bounded(
    items: TokenStream,
    struct_struct: Generator,
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

    let mut types = Vec::new();
    for attr in default.attrs.iter() {
        for token in attr.tokens.clone() {
            if let TokenTree::Group(ref g) = token {
                use syn::parse::Parser;
                let parser = Punctuated::<syn::Type, Comma>::parse_terminated;

                match parser.parse2(g.stream()) {
                    Ok(l) => types.extend(l.into_iter()),
                    Err(err) => return err.to_compile_error().into(),
                }
            } else {
                return darling::Error::unsupported_format("expected bounded_to(...)")
                    .write_errors()
                    .into();
            }
        }
    }

    match default.data {
        darling::ast::Data::Struct(ref fields) => {
            let type_params_in_body = fields
                .iter()
                .collect_type_params(&Purpose::BoundImpl.into(), &type_params);

            let type_params_in_attrs =
                types.collect_type_params(&Purpose::BoundImpl.into(), &type_params);

            let leftovers = type_params_in_body
                .difference(&type_params_in_attrs)
                .map(|&ident| {
                    let path = syn::Path::from(ident.clone());
                    let path = TypePath { qself: None, path };
                    syn::Type::from(path)
                })
                .collect::<Vec<_>>();

            normalize_generics(bound, &mut generics, types.iter().chain(leftovers.iter()));

            match fields.style {
                Style::Struct => {
                    // SAFETY: Struct style struct has always fields
                    let inner = TokenStream2::from_iter(
                        fields
                            .fields
                            .iter()
                            .map(|f| (struct_struct.named_field)(f.ident.as_ref().unwrap())),
                    );
                    (struct_struct.named_body)(&default.ident, generics, inner).into()
                }
                Style::Tuple => {
                    let inner = TokenStream2::from_iter(
                        fields
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(i, _f)| (struct_struct.unnamed_field)(syn::Index::from(i))),
                    );
                    (struct_struct.unnamed_body)(&default.ident, generics, inner).into()
                }
                _ => todo!(),
            }
        }
        darling::ast::Data::Enum(ref _variants) => {
            todo!()
        }
    }
}

/// Derive [Default][std::default::Default]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(Default, attributes(bounded_to))]
pub fn default_bounded(items: TokenStream) -> TokenStream {
    let struct_struct = Generator {
        named_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
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
        },

        unnamed_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::default::Default for #name #ty_generics #where_clause {
                    fn default() -> Self {
                        Self(
                            #inner
                        )
                    }
                }
            }
        },

        named_field: |field: &Ident| -> TokenStream2 {
            quote! { #field: std::default::Default::default(), }
        },

        unnamed_field: |_index| -> TokenStream2 {
            quote! { std::default::Default::default(), }
        },
    };

    let bound = quote! { std::default::Default };

    common_bounded(items, struct_struct, bound)
}

/// Derive [Clone][std::clone::Clone]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(Clone, attributes(bounded_to))]
pub fn clone_bounded(items: TokenStream) -> TokenStream {
    let struct_struct = Generator {
        named_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
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
        },

        unnamed_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::clone::Clone for #name #ty_generics #where_clause {
                    fn clone(&self) -> Self {
                        Self (
                            #inner
                        )
                    }
                }
            }
        },

        named_field: |field: &Ident| -> TokenStream2 {
            quote! { #field: self.#field.clone(), }
        },

        unnamed_field: |index| -> TokenStream2 {
            quote! { self.#index.clone(), }
        },
    };

    let bound = quote! { std::clone::Clone };

    common_bounded(items, struct_struct, bound)
}

/// Derive [Debug][std::fmt::Debug]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(Debug, attributes(bounded_to))]
pub fn debug_bounded(items: TokenStream) -> TokenStream {
    let struct_struct = Generator {
        named_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
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
        },

        unnamed_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            let s = name.to_string();
            quote! {
                impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        f.debug_tuple(#s)
                        #inner
                        .finish()
                    }
                }
            }
        },

        named_field: |field: &Ident| -> TokenStream2 {
            let s = field.to_string();
            quote! { .field(#s, &self.#field) }
        },

        unnamed_field: |index| -> TokenStream2 {
            quote! { .field(&self.#index) }
        },
    };

    let bound = quote! { std::fmt::Debug };

    common_bounded(items, struct_struct, bound)
}

/// Derive [PartialEq][std::cmp::PartialEq]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(PartialEq, attributes(bounded_to))]
pub fn partial_eq_bounded(items: TokenStream) -> TokenStream {
    let struct_struct = Generator {
        named_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::PartialEq for #name #ty_generics #where_clause {
                    fn eq(&self, other: &Self) -> bool {
                        true
                        #inner
                    }
                }
            }
        },

        unnamed_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::PartialEq for #name #ty_generics #where_clause {
                    fn eq(&self, other: &Self) -> bool {
                        true
                        #inner
                    }
                }
            }
        },

        named_field: |field: &Ident| -> TokenStream2 {
            quote! { && other.#field == self.#field }
        },

        unnamed_field: |index| -> TokenStream2 {
            quote! { && other.#index == self.#index }
        },
    };

    let bound = quote! { std::cmp::PartialEq };

    common_bounded(items, struct_struct, bound)
}

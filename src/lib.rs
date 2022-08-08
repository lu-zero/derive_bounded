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
//! - The macro does not auto-generate the where clause for associated traits, e.g. `A` in the
//! example needs the `bounded_to` attribute
//!
//! Later versions will address those.
//!

use std::ops::Not;

use darling::ast::Style;
use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::token::{self, Comma};
use syn::{
    parenthesized, parse_macro_input, parse_quote, DeriveInput, Fields, Generics, Ident,
    PredicateType, TypePath,
};

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

struct BoundedAttr {
    _paren_token: token::Paren,
    types: Punctuated<syn::Type, Comma>,
}

impl Parse for BoundedAttr {
    fn parse(input: ParseStream) -> Result<Self, syn::Error> {
        let content;
        let parent_token = parenthesized!(content in input);
        Ok(BoundedAttr {
            _paren_token: parent_token,
            types: content.parse_terminated(syn::Type::parse)?,
        })
    }
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

        where_clause.predicates.push(pred);
    }
}

struct Generator {
    named_body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,
    unnamed_body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,
    enum_body: fn(name: &Ident, generics: Generics, inner: TokenStream2) -> TokenStream2,

    named_field: fn(field: &Ident) -> TokenStream2,
    unnamed_field: fn(index: syn::Index) -> TokenStream2,
    enum_fields: fn(variant: &syn::Variant) -> TokenStream2,
}

fn variant_fields(prefix: &Ident, fields: &syn::Fields) -> Vec<Ident> {
    match fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|f| {
                let field = f.ident.as_ref().unwrap();
                format_ident!("{prefix}_{field}")
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .enumerate()
            .map(|(i, _)| format_ident!("{prefix}_{i}"))
            .collect(),
        Fields::Unit => Vec::new(),
    }
}

fn unpack_struct(var: &Ident, fields: &syn::Fields) -> TokenStream2 {
    match fields {
        Fields::Named(named) => {
            let args = named
                .named
                .iter()
                .map(|f| f.ident.as_ref().unwrap())
                .zip(variant_fields(var, fields))
                .map(|(v, f)| quote! { #v: #f });
            quote! {
                { #(#args, )* }
            }
        }
        Fields::Unnamed(_) => {
            let args = variant_fields(var, fields);
            quote! {
                ( #(#args, )* )
            }
        }
        Fields::Unit => TokenStream2::new(),
    }
}

fn common_bounded(items: TokenStream, generator: Generator, bound: TokenStream2) -> TokenStream {
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
        match syn::parse2::<BoundedAttr>(attr.tokens.clone()) {
            Ok(ba) => types.extend(ba.types.into_iter()),
            Err(_) => {
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
                            .map(|f| (generator.named_field)(f.ident.as_ref().unwrap())),
                    );
                    (generator.named_body)(&default.ident, generics, inner).into()
                }
                Style::Tuple => {
                    let inner = TokenStream2::from_iter(
                        fields
                            .fields
                            .iter()
                            .enumerate()
                            .map(|(i, _f)| (generator.unnamed_field)(syn::Index::from(i))),
                    );
                    (generator.unnamed_body)(&default.ident, generics, inner).into()
                }
                _ => todo!(),
            }
        }
        darling::ast::Data::Enum(ref variants) => {
            let type_params_in_body = variants
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

            let inner = TokenStream2::from_iter(
                variants
                    .iter()
                    .map(|variant| (generator.enum_fields)(variant)),
            );

            (generator.enum_body)(&default.ident, generics, inner).into()
        }
    }
}

/// Derive [Default][std::default::Default]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(Default, attributes(bounded_to, default))]
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

        enum_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::default::Default for #name #ty_generics #where_clause {
                    fn default() -> Self {
                        Self::#inner
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

        enum_fields: |_variant| -> TokenStream2 {
            darling::Error::unsupported_shape("Enum default is not supported").write_errors()
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

        enum_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::clone::Clone for #name #ty_generics #where_clause {
                    fn clone(&self) -> Self {
                        match self {
                            #inner
                        }
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

        enum_fields: |variant| -> TokenStream2 {
            let self_ident = Ident::new("self", Span::call_site());
            let self_fields = variant_fields(&self_ident, &variant.fields);
            let match_arm = unpack_struct(&self_ident, &variant.fields);
            let variant_ident = &variant.ident;

            let inner = match variant.fields {
                Fields::Named(ref named) => {
                    let inner = TokenStream2::from_iter(
                        named.named.iter().zip(self_fields.iter()).map(|(f, s)| {
                            let f = f.ident.as_ref().unwrap();
                            quote! { #f: #s.clone(), }
                        }),
                    );
                    quote! {
                        Self:: #variant_ident { #inner }
                    }
                }
                Fields::Unnamed(_) => {
                    let inner = TokenStream2::from_iter(self_fields.iter().map(|s| {
                        quote! { #s.clone(), }
                    }));
                    quote! {
                        Self:: #variant_ident ( #inner )
                    }
                }
                Fields::Unit => quote! { Self:: #variant_ident },
            };

            quote! { Self:: #variant_ident #match_arm => #inner, }
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

        enum_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::fmt::Debug for #name #ty_generics #where_clause {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        match self {
                            #inner
                        }
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
        enum_fields: |variant| -> TokenStream2 {
            let self_ident = Ident::new("self", Span::call_site());
            let self_fields = variant_fields(&self_ident, &variant.fields);
            let match_arm = unpack_struct(&self_ident, &variant.fields);
            let variant_ident = &variant.ident;
            let s = variant_ident.to_string();
            let inner = match variant.fields {
                Fields::Named(ref named) => {
                    let inner = TokenStream2::from_iter(
                        named.named.iter().zip(self_fields.iter()).map(|(s, f)| {
                            let s = s.ident.as_ref().unwrap().to_string();
                            quote! { .field(#s, #f) }
                        }),
                    );
                    quote! {
                        f.debug_struct(#s)
                        #inner
                        .finish()
                    }
                }
                Fields::Unnamed(_) => {
                    let inner = TokenStream2::from_iter(self_fields.iter().map(|s| {
                        quote! { .field(#s) }
                    }));
                    quote! {
                          f.debug_tuple(#s)
                          #inner
                          .finish()
                    }
                }
                Fields::Unit => quote! { f.write_str(#s) },
            };

            quote! { Self:: #variant_ident #match_arm => #inner, }
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

        enum_body: |name: &Ident, generics: Generics, inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::PartialEq for #name #ty_generics #where_clause {
                    fn eq(&self, other: &Self) -> bool {
                        match (self, other) {
                            #inner
                            _ => false,
                        }
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

        enum_fields: |variant| -> TokenStream2 {
            let self_ident = Ident::new("self", Span::call_site());
            let self_fields = variant_fields(&self_ident, &variant.fields);
            let other_ident = Ident::new("other", Span::call_site());
            let other_fields = variant_fields(&other_ident, &variant.fields);
            let self_match_arm = unpack_struct(&self_ident, &variant.fields);
            let other_match_arm = unpack_struct(&other_ident, &variant.fields);
            let variant_ident = &variant.ident;
            let variant_ident = quote! { Self:: #variant_ident };

            let match_arm =
                quote! { (#variant_ident #self_match_arm, #variant_ident #other_match_arm) };

            // TODO replace with intersperse
            let inner = TokenStream2::from_iter(other_fields.iter().zip(self_fields.iter()).map(
                |(o, s)| {
                    quote! { && #o == #s }
                },
            ));

            quote! { #match_arm => true #inner, }
        },
    };

    let bound = quote! { std::cmp::PartialEq };

    common_bounded(items, struct_struct, bound)
}

/// Derive [Eq][std::cmp::Eq]
///
/// Use the attribute `#[bounded_to(T, A::B)] to to specify more precise bounds.
#[proc_macro_derive(Eq, attributes(bounded_to))]
pub fn eq_bounded(items: TokenStream) -> TokenStream {
    let struct_struct = Generator {
        named_body: |name: &Ident, generics: Generics, _inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::Eq for #name #ty_generics #where_clause {}
            }
        },

        unnamed_body: |name: &Ident, generics: Generics, _inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::Eq for #name #ty_generics #where_clause {}
            }
        },

        enum_body: |name: &Ident, generics: Generics, _inner| -> TokenStream2 {
            let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

            quote! {
                impl #impl_generics std::cmp::Eq for #name #ty_generics #where_clause {}
            }
        },

        named_field: |_field: &Ident| -> TokenStream2 {
            quote! {}
        },

        unnamed_field: |_index| -> TokenStream2 {
            quote! {}
        },

        enum_fields: |_variant| -> TokenStream2 {
            quote! {}
        },
    };

    let bound = quote! { std::cmp::Eq };

    common_bounded(items, struct_struct, bound)
}

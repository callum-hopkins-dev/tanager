use std::borrow::Borrow;

use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{
    Attribute, Data, DataEnum, DataStruct, DeriveInput, Expr, Field, Fields, GenericParam, Ident,
    Token, Variant, parse_quote,
};

#[proc_macro_derive(Parse, attributes(tanager))]
pub fn derive_parse(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);

    match derive_parse_container(input) {
        Ok(x) => x.into(),
        Err(err) => err.into_compile_error().into(),
    }
}

fn derive_parse_container(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ContainerAttrs { crate_ident } = ContainerAttrs::parse(&input.attrs)?;

    let crate_ident = crate_ident.unwrap_or_else(|| Ident::new("tanager", Span::call_site()));

    let parse_impl = match input.data {
        Data::Struct(x) => derive_parse_struct(&crate_ident, x)?,

        Data::Enum(x) => derive_parse_enum(&crate_ident, x)?,

        Data::Union(_) => {
            return Err(syn::Error::new(
                Span::call_site(),
                "union types are not supported",
            ));
        }
    };

    let mut generics = input.generics;

    for param in &mut generics.params {
        if let GenericParam::Type(ref mut type_param) = *param {
            type_param.bounds.push(parse_quote! { #crate_ident::Parse });
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let ident = input.ident;

    Ok(quote! {
        impl #impl_generics #crate_ident::Parse for #ident #ty_generics #where_clause {
            #parse_impl
        }
    }
    .into_token_stream())
}

#[inline]
fn derive_parse_struct(
    crate_ident: &Ident,
    data: DataStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    match data.fields {
        Fields::Named(_) => derive_parse_struct_named(crate_ident, data),

        Fields::Unnamed(_) if data.fields.len() == 1 => {
            derive_parse_struct_newtype(crate_ident, data)
        }

        Fields::Unnamed(_) => derive_parse_struct_tuple(crate_ident, data),
        Fields::Unit => derive_parse_struct_unit(crate_ident, data),
    }
}

fn derive_parse_enum(crate_ident: &Ident, data: DataEnum) -> syn::Result<proc_macro2::TokenStream> {
    let variants = data.variants.iter().enumerate().map(|(index, variant)| {
        let Variant { ident, fields, .. } = variant;

        let else_token = (index != 0).then_some(quote! { else });

        match fields {
            Fields::Named(fields) => {
                let idents = fields
                    .named
                    .iter()
                    .filter_map(|x| x.ident.as_ref())
                    .map(|ident| quote! { #ident: fields.#ident });

                quote! {
                    #else_token if ident == ::core::stringify!(#ident) {
                        #[derive(#crate_ident::__macro::Parse)]
                        #[tanager(crate = #crate_ident)]
                        struct Fields #fields;

                        let fields = input.call(<Fields as #crate_ident::Parse>::parse)?;

                        return Ok(Self::#ident { #(#idents),* });
                    }
                }
            }

            Fields::Unnamed(fields) => {
                let indices = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(index, _)| proc_macro2::Literal::usize_unsuffixed(index));

                quote! {
                    #else_token if ident == ::core::stringify!(#ident) {
                        #[derive(#crate_ident::__macro::Parse)]
                        #[tanager(crate = #crate_ident)]
                        struct Fields #fields;

                        let fields = input.call(<Fields as #crate_ident::Parse>::parse)?;

                        return Ok(Self::#ident(#(fields.#indices),*));
                    }
                }
            }

            Fields::Unit => quote! {
                #else_token if ident == ::core::stringify!(#ident) {
                    return Ok(Self::#ident);
                }
            },
        }
    });

    Ok(quote! {
        fn parse(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            let ident = input.parse::<#crate_ident::__macro::syn::Ident>()?;

            #(#variants)*

            Err(#crate_ident::__macro::syn::Error::new(
                ident.span(),
                #crate_ident::__macro::format!("unknown variant `{ident}`"),
            ))
        }
    })
}

fn derive_parse_struct_named(
    crate_ident: &Ident,
    data: DataStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let builder_fields = data.fields.iter().map(|field| {
        let Field { ident, ty, .. } = field;

        quote! {
            let mut #ident: ::core::option::Option<#ty> = ::core::option::Option::None;
        }
    });

    let parse_fields = data.fields.iter().enumerate().map(|(index, field)| {
        let Field { ident, ty, .. } = field;

        let else_token = (index != 0).then_some(quote! { else });

        quote! {
            #else_token if ident == ::core::stringify!(#ident) {
                if #ident.is_none() {
                    #ident = ::core::option::Option::Some(
                        input.call(<#ty as #crate_ident::Parse>::parse)?,
                    );

                    continue;
                } else {
                    return Err(#crate_ident::__macro::syn::Error::new(
                        ident.span(),
                        ::core::concat!("duplicate field `", ::core::stringify!(#ident), "`"),
                    ));
                }
            }
        }
    });

    let unwrap_fields = data
        .fields
        .iter()
        .map(|field| {
            let Field { attrs, ident, .. } = field;

            let NamedFieldAttrs { default } = NamedFieldAttrs::parse(attrs)?;

            match default {
                Some(default) => Ok(quote! {
                    #ident: #ident.unwrap_or_else(|| #default)
                }),

                None => Ok(quote! {
                    #ident: #ident.ok_or_else(|| #crate_ident::__macro::syn::Error::new(
                        input_span,
                        ::core::concat!("missing field `", ::core::stringify!(#ident), "`"),
                    ))?
                }),
            }
        })
        .collect::<syn::Result<Box<[_]>>>()?;

    Ok(quote! {
        fn parse(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            let inner;
            #crate_ident::__macro::syn::braced!(inner in input);

            inner.call(|x| Self::parse_without_container(x))
        }

        fn parse_without_container(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            let input_span = input.span();

            let mut first = true;

            #(#builder_fields)*

            while !input.is_empty() {
                if first {
                    first = false;
                } else {
                    let _ = input.parse::<#crate_ident::__macro::syn::Token![,]>()?;

                    if input.is_empty() {
                        break;
                    }
                }

                let ident = input.parse::<#crate_ident::__macro::syn::Ident>()?;
                let _ = input.parse::<#crate_ident::__macro::syn::Token![:]>()?;

                #(#parse_fields)*

                return Err(#crate_ident::__macro::syn::Error::new(
                    ident.span(),
                    #crate_ident::__macro::format!("invalid field `{ident}`"),
                ));
            }

            let _ = input.parse::<::core::option::Option<#crate_ident::__macro::syn::Token![,]>>()?;

            Ok(Self {
                #(#unwrap_fields),*
            })
        }
    })
}

fn derive_parse_struct_tuple(
    crate_ident: &Ident,
    data: DataStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let fields = data
        .fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let Field { attrs, ty, .. } = field;

            let _ = UnnamedFieldAttrs::parse(attrs)?;

            if index == 0 {
                Ok(quote! { input.call(<#ty as #crate_ident::Parse>::parse)? })
            } else {
                Ok(quote! {{
                    let x = input.call(<#ty as #crate_ident::Parse>::parse)?;
                    let _ = input.parse::<#crate_ident::__macro::syn::Token![,]>()?;
                    x
                }})
            }
        })
        .collect::<syn::Result<Box<[_]>>>()?;

    Ok(quote! {
        #[inline]
        fn parse(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            let inner;
            #crate_ident::__macro::syn::parenthesized!(inner in input);

            inner.call(|x| Self::parse_without_container(x))
        }

        fn parse_without_container(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            let x = Self(#(#fields),*);
            let _ = input.parse::<::core::option::Option<#crate_ident::__macro::syn::Token![,]>>()?;

            Ok(x)
        }
    })
}

fn derive_parse_struct_newtype(
    crate_ident: &Ident,
    _data: DataStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    Ok(quote! {
        #[inline]
        fn parse(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            Ok(Self(input.call(|x| #crate_ident::Parse::parse(x))?))
        }

        #[inline]
        fn parse_without_container(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            Ok(Self(input.call(|x| #crate_ident::Parse::parse_without_container(x))?))
        }
    })
}

fn derive_parse_struct_unit(
    crate_ident: &Ident,
    _data: DataStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    Ok(quote! {
        fn parse(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            Ok(Self)
        }

        fn parse_without_container(input: #crate_ident::ParseStream<'_>) -> #crate_ident::Result<Self> {
            Ok(Self)
        }
    })
}

struct ContainerAttrs {
    crate_ident: Option<Ident>,
}

impl ContainerAttrs {
    fn parse<I>(attrs: I) -> syn::Result<Self>
    where
        I: IntoIterator<Item: Borrow<Attribute>>,
    {
        let mut container_attrs = Self { crate_ident: None };

        for attr in attrs
            .into_iter()
            .filter(|x| x.borrow().path().is_ident("tanager"))
        {
            attr.borrow().parse_nested_meta(|meta| {
                if meta.path.is_ident("crate") {
                    if container_attrs.crate_ident.is_none() {
                        container_attrs.crate_ident = Some(meta.value()?.parse()?);
                        Ok(())
                    } else {
                        Err(meta.error("duplicate attribute `crate`"))
                    }
                } else {
                    Err(meta.error("unrecognised attribute"))
                }
            })?;
        }

        Ok(container_attrs)
    }
}

struct NamedFieldAttrs {
    default: Option<Expr>,
}

impl NamedFieldAttrs {
    fn parse<I>(attrs: I) -> syn::Result<Self>
    where
        I: IntoIterator<Item: Borrow<Attribute>>,
    {
        let mut named_field_attrs = Self { default: None };

        for attr in attrs
            .into_iter()
            .filter(|x| x.borrow().path().is_ident("tanager"))
        {
            attr.borrow().parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    if named_field_attrs.default.is_none() {
                        if meta.input.peek(Token![=]) {
                            named_field_attrs.default = Some(meta.value()?.parse()?);

                            Ok(())
                        } else {
                            named_field_attrs.default =
                                Some(parse_quote!(::core::default::Default::default()));

                            Ok(())
                        }
                    } else {
                        Err(meta.error("duplicate attribute `default`"))
                    }
                } else {
                    Err(meta.error("unrecognised attribute"))
                }
            })?;
        }

        Ok(named_field_attrs)
    }
}

struct UnnamedFieldAttrs;

impl UnnamedFieldAttrs {
    fn parse<I>(attrs: I) -> syn::Result<Self>
    where
        I: IntoIterator<Item: Borrow<Attribute>>,
    {
        for attr in attrs
            .into_iter()
            .filter(|x| x.borrow().path().is_ident("tanager"))
        {
            attr.borrow().parse_nested_meta(|meta| {
                if meta.path.is_ident("default") {
                    Err(meta.error("attribute `default` is not supported for tuples"))
                } else {
                    Err(meta.error("unrecognised attribute"))
                }
            })?;
        }

        Ok(UnnamedFieldAttrs)
    }
}

//! Parse Rust literal struct expressions using `syn`.
//!
//! `tanager` is intended for proc-macro authors who want to accept syntax that
//! resembles ordinary Rust struct expressions.
//!
//! The crate provides a [`Parse`] trait and a derive macro for implementing it.
//! Parsed values are constructed directly from Rust literals and nested
//! structures rather than from custom attribute syntax.
//!
//! # Example
//!
//! ```rust
//! use tanager::Parse;
//!
//! #[derive(Parse)]
//! struct Config {
//!     name: String,
//!     enabled: bool,
//! }
//!
//! // Parses:
//! // {
//! //     name: "example",
//! //     enabled: true,
//! // }
//! ```
//!
//! Most users will derive [`Parse`] and then use [`parse`] to parse a
//! `proc_macro2::TokenStream`.
//!
//! The derive macro is available with the `macros` feature enabled.

use std::{
    ffi::{CStr, CString},
    rc::Rc,
    sync::Arc,
};

use syn::{
    LitBool, LitByte, LitByteStr, LitCStr, LitChar, LitFloat, LitInt, LitStr, Token, parenthesized,
    token::Bracket,
};

/// Result type used throughout `tanager` parsing APIs.
pub use syn::{Result, parse::ParseStream};

/// Derive [`Parse`] for a struct or enum.
///
/// The generated implementation parses values from Rust-like literal syntax.
///
/// # Container attributes
///
/// ## `#[tanager(crate = path)]`
///
/// Overrides the path used to refer to the `tanager` crate in generated code.
///
/// This is primarily useful when re-exporting `tanager` or when the crate is
/// available under a different name.
///
/// ```rust
/// # use ::tanager as my_tanager;
/// # use tanager::Parse;
/// #[derive(Parse)]
/// #[tanager(crate = my_tanager)]
/// struct Config {
///     enabled: bool,
/// }
/// ```
///
/// # Field attributes
///
/// ## `#[tanager(default = expr)]`
///
/// Provides a default value when a named field is omitted from the input.
///
/// ```rust
/// # use tanager::Parse;
/// #[derive(Parse)]
/// struct Config {
///     required: String,
///
///     #[tanager(default = true)]
///     enabled: bool,
/// }
/// ```
///
/// In this example, `enabled` defaults to `true` if the field is not present
/// in the parsed input.
///
/// Available when the `macros` feature is enabled.
#[cfg(feature = "macros")]
pub use ::tanager_macros::Parse;

/// Parses a value from a Rust literal expression.
///
/// This trait is primarily intended to be derived using
/// `#[derive(Parse)]`.
///
/// Implementations are provided for common primitive types, strings,
/// C strings, collections, and `Option`.
pub trait Parse
where
    Self: Sized,
{
    /// Parses a value from the input stream.
    fn parse(input: ParseStream<'_>) -> crate::Result<Self>;

    /// Parses a value without requiring its usual outer container syntax.
    ///
    /// This is primarily used when parsing top-level values and generally does
    /// not need to be implemented manually.
    #[inline]
    fn parse_without_container(input: ParseStream<'_>) -> crate::Result<Self> {
        Self::parse(input)
    }

    #[doc(hidden)]
    #[inline]
    fn parse_seq(input: ParseStream<'_>) -> crate::Result<Vec<Self>> {
        let inner;
        syn::bracketed!(inner in input);

        inner.call(|x| Self::parse_seq_without_container(x))
    }

    #[doc(hidden)]
    #[inline]
    fn parse_seq_without_container(input: ParseStream<'_>) -> crate::Result<Vec<Self>> {
        Ok(input
            .parse_terminated(Self::parse, Token![,])?
            .into_iter()
            .collect())
    }
}

impl Parse for u8 {
    #[inline]
    fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
        let lookahead = input.lookahead1();

        if lookahead.peek(LitInt) {
            input.parse::<LitInt>().and_then(|x| x.base10_parse())
        } else if lookahead.peek(LitByte) {
            input.parse::<LitByte>().map(|x| x.value())
        } else {
            Err(lookahead.error())
        }
    }

    fn parse_seq(input: ParseStream<'_>) -> crate::Result<Vec<Self>> {
        let lookahead = input.lookahead1();

        if lookahead.peek(LitByteStr) {
            input.parse::<LitByteStr>().map(|x| x.value())
        } else if lookahead.peek(Bracket) {
            let inner;
            syn::bracketed!(inner in input);

            Ok(inner
                .parse_terminated(Self::parse, Token![,])?
                .into_iter()
                .collect())
        } else {
            Err(lookahead.error())
        }
    }

    fn parse_seq_without_container(input: ParseStream<'_>) -> crate::Result<Vec<Self>> {
        let lookahead = input.lookahead1();

        if lookahead.peek(LitByteStr) {
            input.parse::<LitByteStr>().map(|x| x.value())
        } else if lookahead.peek(Bracket) {
            Ok(input
                .parse_terminated(Self::parse, Token![,])?
                .into_iter()
                .collect())
        } else {
            Err(lookahead.error())
        }
    }
}

macro_rules! impl_integer {
    ($ty:ty) => {
        impl Parse for $ty {
            #[inline]
            fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
                input.parse::<LitInt>().and_then(|x| x.base10_parse())
            }
        }
    };
}

impl_integer!(usize);
impl_integer!(isize);

impl_integer!(i8);

impl_integer!(u16);
impl_integer!(i16);

impl_integer!(u32);
impl_integer!(i32);

impl_integer!(u64);
impl_integer!(i64);

impl_integer!(u128);
impl_integer!(i128);

macro_rules! impl_float {
    ($ty:ty) => {
        impl Parse for $ty {
            #[inline]
            fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
                input.parse::<LitFloat>().and_then(|x| x.base10_parse())
            }
        }
    };
}

impl_float!(f32);
impl_float!(f64);

macro_rules! impl_generic {
    ($lit:ty as $ty:ty) => {
        impl Parse for $ty {
            #[inline]
            fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
                input.parse::<$lit>().map(|x| x.value().into())
            }
        }
    };
}

impl_generic!(LitBool as bool);
impl_generic!(LitChar as char);

impl_generic!(LitStr as String);
impl_generic!(LitStr as Box<str>);
impl_generic!(LitStr as Rc<str>);
impl_generic!(LitStr as Arc<str>);

impl_generic!(LitCStr as CString);
impl_generic!(LitCStr as Box<CStr>);
impl_generic!(LitCStr as Rc<CStr>);
impl_generic!(LitCStr as Arc<CStr>);

macro_rules! impl_seq {
    ($ty:ty) => {
        impl<T> Parse for $ty
        where
            T: Parse,
        {
            #[inline]
            fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
                T::parse_seq(input).map(|x| x.into())
            }

            #[inline]
            fn parse_without_container(input: ParseStream<'_>) -> crate::Result<Self> {
                T::parse_seq_without_container(input).map(|x| x.into())
            }
        }
    };
}

impl_seq!(Vec<T>);
impl_seq!(Box<[T]>);
impl_seq!(Rc<[T]>);
impl_seq!(Arc<[T]>);

impl<T> Parse for Option<T>
where
    T: Parse,
{
    fn parse(input: ParseStream<'_>) -> crate::Result<Self> {
        mod kw {
            syn::custom_keyword!(None);
            syn::custom_keyword!(Some);
        }

        if input.peek(kw::Some) {
            let _ = input.parse::<kw::Some>()?;

            let inner;
            parenthesized!(inner in input);

            Ok(Some(inner.call(T::parse)?))
        } else if input.peek(kw::None) {
            let _ = input.parse::<kw::None>()?;
            Ok(None)
        } else {
            Ok(Some(input.call(T::parse)?))
        }
    }
}

/// Parses a value from a token stream.
///
/// This is a convenience wrapper around [`Parse`] implementations.
///
/// # Errors
///
/// Returns any parsing error produced by `T::parse` or by `syn` while parsing
/// the token stream.
#[inline]
pub fn parse<T>(tokens: proc_macro2::TokenStream) -> syn::Result<T>
where
    T: Parse,
{
    struct Parser<T>(T);

    impl<T> syn::parse::Parse for Parser<T>
    where
        T: Parse,
    {
        #[inline]
        fn parse(input: ParseStream) -> Result<Self> {
            input.call(|x| T::parse(x)).map(|x| Self(x))
        }
    }

    syn::parse2::<Parser<T>>(tokens).map(|x| x.0)
}

/// Parses a value from a token stream without requiring its outer container.
///
/// This is equivalent to calling [`Parse::parse_without_container`] on the
/// target type.
#[inline]
pub fn parse_without_container<T>(tokens: proc_macro2::TokenStream) -> syn::Result<T>
where
    T: Parse,
{
    struct Parser<T>(T);

    impl<T> syn::parse::Parse for Parser<T>
    where
        T: Parse,
    {
        #[inline]
        fn parse(input: ParseStream) -> Result<Self> {
            input
                .call(|x| T::parse_without_container(x))
                .map(|x| Self(x))
        }
    }

    syn::parse2::<Parser<T>>(tokens).map(|x| x.0)
}

/// Support items used by generated code.
///
/// This module is not part of the `tanager` public API and may change without
/// notice.
#[cfg(feature = "macros")]
#[doc(hidden)]
pub mod __macro {
    pub use ::std::format;
    pub use ::tanager_macros::Parse;
    pub use syn;
}

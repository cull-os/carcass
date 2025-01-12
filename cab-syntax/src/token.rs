//! [`Token`] definitions for the Cab language.
use std::{
    fmt,
    ops,
};

use num::Num;

use crate::{
    Kind::{
        self,
        *,
    },
    RowanToken,
};

pub trait Token: ops::Deref<Target = RowanToken> {
    /// Determines if this token can be created from this Kind.
    fn can_cast(from: Kind) -> bool;

    /// Casts a RowanToken to this Token. Returns None if it can't.
    fn cast(from: RowanToken) -> Option<Self>
    where
        Self: Sized;

    /// Returns the underlying RowanToken.
    fn syntax(&self) -> &RowanToken;
}

macro_rules! token {
    (#[from($kind:ident)]struct $name:ident;) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) RowanToken);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self.syntax(), formatter)
            }
        }

        impl Token for $name {
            fn can_cast(kind: Kind) -> bool {
                kind == $kind
            }

            fn cast(from: RowanToken) -> Option<Self> {
                Self::can_cast(from.kind()).then_some(Self(from))
            }

            fn syntax(&self) -> &RowanToken {
                &self.0
            }
        }

        impl ops::Deref for $name {
            type Target = RowanToken;

            fn deref(&self) -> &Self::Target {
                self.syntax()
            }
        }
    };
}

token! { #[from(TOKEN_ERROR)] struct Error; }

token! { #[from(TOKEN_WHITESPACE)] struct Whitespace; }

impl Whitespace {
    /// The newline count of this whitespace.
    pub fn newline_count(&self) -> usize {
        self.text().lines().count() - 1
    }
}

token! { #[from(TOKEN_COMMENT)] struct Comment; }

impl Comment {
    /// Returns the delimiter of this comment.
    pub fn delimiter(&self) -> &str {
        let text = self.text();
        let content_start_index = text.find(|c| c != '#').unwrap_or(text.len());

        &text[..content_start_index]
    }

    /// Returns whether if this comment has the capability to span multiple
    /// lines.
    pub fn multiline(&self) -> bool {
        self.delimiter().len() >= 3
    }

    /// Returns whether if this multiline comment was closed off properly.
    ///
    /// Panics if this comment is not a multiline comment.
    pub fn closed_off(&self) -> bool {
        assert!(self.multiline());
        self.text().ends_with(self.delimiter())
    }

    /// Returns the contents of this comment by removing delimiters.
    pub fn contents(&self) -> &str {
        let delimiter = self.delimiter();

        let start_stripped = self.text().strip_prefix(delimiter).unwrap();

        if self.multiline() {
            start_stripped
                .strip_suffix(delimiter)
                // Not .unwrap(), because it's perfectly fine if you don't close off your multiline string.
                .unwrap_or(start_stripped)
        } else {
            start_stripped
        }
    }
}

token! { #[from(TOKEN_INTEGER)] struct Integer; }

impl Integer {
    /// Returns the value of this integer, after resolving binary,
    /// octadecimal and hexadecimal notation if it exists.
    ///
    /// Will panic if the underlying token is not valid.
    pub fn value(&self) -> num::BigInt {
        let text = self.text();

        match text.chars().nth(1) {
            Some('b') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 2),
            Some('o') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 8),
            Some('x') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 16),
            _ => num::BigInt::from_str_radix(text, 10),
        }
        .unwrap()
    }
}

token! { #[from(TOKEN_FLOAT)] struct Float; }

impl Float {
    /// Returns the value of the float by parsing the underlying string.
    pub fn value(&self) -> f64 {
        let text = self.text();

        match text.chars().nth(1) {
            Some('b') => f64::from_str_radix(text.get(2..).unwrap(), 2),
            Some('o') => f64::from_str_radix(text.get(2..).unwrap(), 8),
            Some('x') => f64::from_str_radix(text.get(2..).unwrap(), 16),
            _ => f64::from_str_radix(text, 10),
        }
        .unwrap()
    }
}

token! { #[from(TOKEN_PATH)] struct Path; }

token! { #[from(TOKEN_IDENTIFIER)] struct Identifier; }

token! { #[from(TOKEN_CONTENT)] struct Content; }

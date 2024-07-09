use std::{
    fmt,
    ops::Deref,
};

use crate::{
    Kind,
    Kind::*,
    RowanToken,
};

pub trait Token: Deref<Target = RowanToken> {
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

        impl Deref for $name {
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
    pub fn delimiter(&self) -> &str {
        let text = self.text();
        let content_start_index = text.find(|c| c != '#').unwrap_or(text.len());

        &text[..content_start_index]
    }

    pub fn multiline(&self) -> bool {
        self.delimiter().len() >= 3
    }

    pub fn contents(&self) -> &str {
        let delimiter = self.delimiter();

        self.text()
            .strip_prefix(delimiter)
            .and_then(|s| {
                self.multiline()
                    .then(|| s.strip_suffix(delimiter))
                    .unwrap_or(Some(s))
            })
            .unwrap()
    }
}

token! { #[from(TOKEN_INTEGER)] struct Integer; }

impl Integer {
    pub fn value(&self) -> i64 {
        let text = self.text();

        match text.chars().nth(1) {
            Some('b') => i64::from_str_radix(text.get(2..).unwrap(), 2).unwrap(),
            Some('o') => i64::from_str_radix(text.get(2..).unwrap(), 8).unwrap(),
            Some('x') => i64::from_str_radix(text.get(2..).unwrap(), 16).unwrap(),
            _ => text.parse().unwrap(),
        }
    }
}

token! { #[from(TOKEN_FLOAT)] struct Float; }

impl Float {
    pub fn value(&self) -> f64 {
        self.text().parse().unwrap()
    }
}

token! { #[from(TOKEN_PATH)] struct Path; }

token! { #[from(TOKEN_IDENTIFIER)] struct Identifier; }

token! { #[from(TOKEN_CONTENT)] struct Content; }

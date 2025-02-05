//! [`Token`] definitions.
use std::{
    fmt,
    ops::{
        self,
        Deref as _,
    },
};

use num::Num;
use static_assertions::assert_obj_safe;

use crate::{
    Kind::{
        self,
        *,
    },
    NodeError,
    RowanToken,
};

assert_obj_safe!(Token);
pub trait Token: ops::Deref<Target = RowanToken> {
    /// Determines if this token can be created from this Kind.
    fn can_cast(from: Kind) -> bool
    where
        Self: Sized;

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
                fmt::Display::fmt(self.deref(), formatter)
            }
        }

        impl Token for $name {
            fn can_cast(kind: Kind) -> bool {
                kind == $kind
            }

            fn cast(token: RowanToken) -> Option<Self> {
                Self::can_cast(token.kind()).then_some(Self(token))
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

token! { #[from(TOKEN_WHITESPACE)] struct Whitespace; }

impl Whitespace {
    /// The newline count of this whitespace.
    pub fn newline_count(&self) -> usize {
        self.text().bytes().filter(|&c| c == b'\n').count()
    }
}

token! { #[from(TOKEN_COMMENT)] struct Comment; }

impl Comment {
    /// Returns the starting delimiter of this comment.
    pub fn start_delimiter(&self) -> &str {
        let text = self.text();
        let content_start_index = text.bytes().skip(1).take_while(|&c| c == b'=').count() + 1;

        &text[..content_start_index]
    }

    /// Returns whether if this comment has the capability to span multiple
    /// lines.
    pub fn is_multiline(&self) -> bool {
        self.text()
            .as_bytes()
            .get(1)
            .copied()
            .is_some_and(|c| c == b'=')
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

        match text.as_bytes().get(1).copied() {
            Some(b'b') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 2),
            Some(b'o') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 8),
            Some(b'x') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 16),
            _ => num::BigInt::from_str_radix(text, 10),
        }
        .unwrap()
    }
}

token! { #[from(TOKEN_FLOAT)] struct Float; }

impl Float {
    /// Returns the value of the float by parsing the underlying slice.
    pub fn value(&self) -> f64 {
        let text = self.text();

        match text.as_bytes().get(1).copied() {
            Some(b'b') => f64::from_str_radix(text.get(2..).unwrap(), 2),
            Some(b'o') => f64::from_str_radix(text.get(2..).unwrap(), 8),
            Some(b'x') => f64::from_str_radix(text.get(2..).unwrap(), 16),
            _ => f64::from_str_radix(text, 10),
        }
        .unwrap()
    }
}

token! { #[from(TOKEN_PATH)] struct Path; }

token! { #[from(TOKEN_IDENTIFIER)] struct Identifier; }

token! { #[from(TOKEN_CONTENT)] struct Content; }

impl Content {
    pub fn validate_escapes(&self, to: &mut Vec<NodeError>) -> usize {
        let mut text = self.text().bytes().enumerate();
        let mut count: usize = 0;

        while let Some((offset, c)) = text.next() {
            if c != b'\\' {
                continue;
            }

            count += 1;

            match text.next() {
                Some((_, b'0' | b't' | b'n' | b'r' | b'`' | b'"' | b'\'' | b'>' | b'\\')) => {},

                next @ (Some(_) | None) => {
                    to.push(NodeError::new(
                        r#"invalid escape, escapes must be one of: \0, \t, \n, \r, \`, \", \', \>, \\"#,
                        rowan::TextRange::at(
                            self.text_range().start() + rowan::TextSize::new(offset as u32),
                            (1 + next.is_some() as u32).into(),
                        ),
                    ));
                },
            }
        }

        count
    }
}

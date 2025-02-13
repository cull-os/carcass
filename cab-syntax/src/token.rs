//! [`Token`] definitions.
use std::{
    fmt,
    ops,
};

use cab_report::{
    Label,
    Report,
};
use cab_text::{
    Range,
    Rangeable,
};
use enumset::enum_set;

// use num::Num;
use crate::{
    FromRed,
    Kind::*,
    Kinds,
    red,
};

macro_rules! token {
    (#[from($kind:ident)]struct $name:ident;) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) red::Token);

        impl fmt::Display for $name {
            fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
                (*self).fmt(writer)
            }
        }

        impl ops::Deref for $name {
            type Target = red::Token;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl FromRed<red::Token> for $name {
            const KINDS: Kinds = enum_set!($kind);

            fn can_cast(token: &red::Token) -> bool {
                Self::KINDS.contains(token.kind())
            }

            fn cast(token: red::Token) -> Option<Self> {
                Self::can_cast(token.kind()).then_some(Self(token))
            }
        }
    };
}

token! { #[from(TOKEN_WHITESPACE)] struct Whitespace; }

impl Whitespace {
    /// Returns the amount of lines this whitespace.
    pub fn newline_count(&self) -> usize {
        self.text().bytes().filter(|&c| c == b'\n').count() + 1
    }
}

token! { #[from(TOKEN_COMMENT)] struct Comment; }

impl Comment {
    const START_HASHTAG_LEN: usize = 1;

    /// Returns the starting delimiter of this comment.
    pub fn start_delimiter(&self) -> &str {
        let text = self.text();

        let content_start_index = text
            .bytes()
            .skip(Self::START_HASHTAG_LEN)
            .take_while(|&c| c == b'=')
            .count()
            + Self::START_HASHTAG_LEN;

        &text[..content_start_index]
    }

    /// Returns whether if this comment has the capability to span multiple
    /// lines.
    pub fn is_multiline(&self) -> bool {
        self.text()
            .as_bytes()
            .get(Self::START_HASHTAG_LEN)
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
            Some(b'b' | b'B') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 2),
            Some(b'o' | b'O') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 8),
            Some(b'x' | b'X') => num::BigInt::from_str_radix(text.get(2..).unwrap(), 16),
            _ => num::BigInt::from_str_radix(text, 10),
        }
        .expect("invalid interger token")
    }
}

token! { #[from(TOKEN_FLOAT)] struct Float; }

impl Float {
    /// Returns the value of the float by parsing the underlying slice.
    pub fn value(&self) -> f64 {
        let text = self.text();

        match text.as_bytes().get(1).copied() {
            Some(b'b' | b'B') => f64::from_str_radix(text.get(2..).unwrap(), 2),
            Some(b'o' | b'O') => f64::from_str_radix(text.get(2..).unwrap(), 8),
            Some(b'x' | b'X') => f64::from_str_radix(text.get(2..).unwrap(), 16),
            _ => f64::from_str_radix(text, 10),
        }
        .expect("invalid float token")
    }
}

token! { #[from(TOKEN_PATH_CONTENT)] struct PathContent; }

token! { #[from(TOKEN_IDENTIFIER)] struct Identifier; }

token! { #[from(TOKEN_CONTENT)] struct Content; }

impl Content {
    pub fn validate_escapes(&self, report: &mut Report<'_>) -> usize {
        let mut reported = false;

        let mut bytes = self.text().bytes().enumerate();
        let mut count: usize = 0;

        while let Some((offset, c)) = bytes.next() {
            if c != b'\\' {
                continue;
            }

            count += 1;

            match bytes.next() {
                Some((_, b'0' | b't' | b'n' | b'r' | b'`' | b'"' | b'\'' | b'>' | b'\\')) => {},

                next @ (Some(_) | None) if !reported => {
                    reported = true;

                    let start = self.range().start + offset;
                    let len = 1 + next.is_some() as u32;
                    report.push_label(Label::primary(Range::at(start, len), "invalid escape"));
                    report.push_tip(r#"escapes must be one of: \0, \t, \n, \r, \`, \", \', \>, \\"#);
                },

                _ => {},
            }
        }

        count
    }
}

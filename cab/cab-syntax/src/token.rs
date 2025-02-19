//! Typed [`Token`] definitions.
//!
//! [`Token`]: crate::Token
use std::{
    fmt,
    mem,
    ops,
};

use cab_report::{
    Label,
    Report,
};
use cab_text::{
    IntoSpan,
    Span,
};
use num::Num as _;

use crate::{
    Kind::*,
    red,
};

macro_rules! token {
    (#[from($kind:ident)]struct $name:ident;) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(red::Token);

        impl fmt::Display for $name {
            fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
                (&**self).fmt(writer)
            }
        }

        impl ops::Deref for $name {
            type Target = red::Token;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'a> TryFrom<&'a red::Token> for &'a $name {
            type Error = ();

            fn try_from(token: &'a red::Token) -> Result<Self, ()> {
                if token.kind() != $kind {
                    return Err(());
                }

                // SAFETY: `token` is an immutable reference and Self is a &red::Token with
                // #[repr(transparent)].
                Ok(unsafe { mem::transmute::<&red::Token, Self>(token) })
            }
        }

        impl TryFrom<red::Token> for $name {
            type Error = ();

            fn try_from(token: red::Token) -> Result<Self, ()> {
                if token.kind() != $kind {
                    return Err(());
                }

                Ok(Self(token))
            }
        }
    };
}

// WHITESPACE

token! { #[from(TOKEN_WHITESPACE)] struct Whitespace; }

impl Whitespace {
    /// Returns the amount of lines this whitespace.
    pub fn newline_count(&self) -> usize {
        self.text().bytes().filter(|&c| c == b'\n').count() + 1
    }
}

// COMMENT

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

// INTEGER

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

// FLOAT

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

// PATH CONTENT

token! { #[from(TOKEN_PATH_CONTENT)] struct PathContent; }

// IDENTIFIER

token! { #[from(TOKEN_IDENTIFIER)] struct Identifier; }

// CONTENT

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentPart<'a> {
    Literal(&'a str),
    Escape(char),
}

token! { #[from(TOKEN_CONTENT)] struct Content; }

impl Content {
    pub fn parts(&self, report: &mut Report) -> impl Iterator<Item = ContentPart<'_>> {
        gen {
            let mut reported = false;

            let mut literal_start_offset = 0;

            let text = self.text();
            let mut bytes = text.char_indices();

            while let Some((offset, c)) = bytes.next() {
                if c != '\\' {
                    let literal = &text[literal_start_offset..offset + c.len_utf8()];

                    if !literal.is_empty() {
                        yield ContentPart::Literal(literal);
                    }

                    continue;
                }

                literal_start_offset = offset;

                yield ContentPart::Escape(match bytes.next() {
                    Some((_, '0')) => '\0',
                    Some((_, 't')) => '\t',
                    Some((_, 'n')) => '\n',
                    Some((_, 'r')) => '\r',
                    Some((_, '`')) => '`',
                    Some((_, '"')) => '"',
                    Some((_, '\'')) => '\'',
                    Some((_, '>')) => '>',
                    Some((_, '\\')) => '\\',

                    next @ (Some(_) | None) if !reported => {
                        reported = true;

                        report.push_label(Label::primary(
                            Span::at(self.span().start + offset, 1 + next.is_some() as u32),
                            "invalid escape",
                        ));

                        report.push_tip(r#"escapes must be one of: \0, \t, \n, \r, \`, \", \', \>, \\"#);

                        continue;
                    },

                    _ => continue,
                });
            }
        }
    }
}

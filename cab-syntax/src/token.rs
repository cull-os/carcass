use std::fmt;

use crate::{
    Kind,
    Kind::*,
    RowanToken,
};

pub trait Token {
    fn can_cast(from: Kind) -> bool;

    fn cast(from: RowanToken) -> Option<Self>
    where
        Self: Sized;

    fn syntax(&self) -> &RowanToken;
}

macro_rules! token {
    (
        #[from($kind:ident)]
        $visibility:vis struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $visibility struct $name(pub RowanToken);

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
    };
}

token! { #[from(TOKEN_WHITESPACE)] pub struct Whitespace; }

token! { #[from(TOKEN_COMMENT)] pub struct Comment; }

impl Comment {
    pub fn text(&self) -> &str {
        let raw = self.syntax().text();
        let start_index = raw.find(|c| c != '#').unwrap_or(raw.len());

        match start_index {
            0 => unreachable!(),
            1..=2 => &raw[start_index..],
            3.. => {
                #[cfg(debug_assertions)]
                {
                    let delimiter = &raw[start_index..];

                    assert!(raw.len() > delimiter.len() * 2);
                    assert!(raw.starts_with(delimiter));
                    assert!(raw.ends_with(delimiter));
                }

                &raw[start_index..raw.len() - start_index]
            },
        }
    }
}

token! { #[from(TOKEN_INTEGER)] pub struct Integer; }

impl Integer {
    pub fn value(&self) -> i64 {
        self.syntax().text().parse().unwrap()
    }
}

token! { #[from(TOKEN_FLOAT)] pub struct Float; }

impl Float {
    pub fn value(&self) -> f64 {
        self.syntax().text().parse().unwrap()
    }
}

token! { #[from(TOKEN_PATH)] pub struct PathContent; }

token! { #[from(TOKEN_IDENTIFIER)] pub struct IdentifierSimple; }

token! { #[from(TOKEN_CONTENT)] pub struct Content; }

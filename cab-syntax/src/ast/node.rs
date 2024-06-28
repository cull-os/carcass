use std::fmt;

use super::Token;
use crate::{
    syntax::{
        self,
        Kind::*,
    },
    Language,
};

pub trait Node: rowan::ast::AstNode<Language = Language> {
    fn nth<N: Node>(&self, n: usize) -> Option<N> {
        self.syntax().children().filter_map(N::cast).nth(n)
    }

    fn children<N: Node>(&self) -> rowan::ast::AstChildren<N> {
        rowan::ast::support::children(self.syntax())
    }

    fn token<T: Token>(&self) -> Option<T> {
        self.children_tokens().next()
    }

    fn token_untyped(&self, kind: syntax::Kind) -> Option<syntax::Token> {
        self.children_tokens_untyped().find(|it| it.kind() == kind)
    }

    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T> {
        self.syntax()
            .children_with_tokens()
            .filter_map(syntax::Element::into_token)
            .filter_map(T::cast)
    }

    fn children_tokens_untyped(&self) -> impl Iterator<Item = syntax::Token> {
        self.syntax()
            .children_with_tokens()
            .filter_map(syntax::Element::into_token)
    }
}

impl<T: rowan::ast::AstNode<Language = Language>> Node for T {}

// pub trait AttributeSet: Node
// where
//     Self: Sized,
// {
//     fn entries(&self) -> rowan::ast::AstChildren<Attribute>
//     where
//         Self: Sized,
//     {
//         self.children()
//     }

//     fn inherits(&self) -> rowan::ast::AstChildren<AttributeInherit>
//     where
//         Self: Sized,
//     {
//         self.children()
//     }
// }

macro_rules! node {
    (
        #[from($kind:ident)]
        $(#[$meta:meta])*
        struct $name:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub syntax::Node);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: syntax::Kind) -> bool {
                kind == $kind
            }

            fn cast(from: syntax::Node) -> Option<Self> {
                Self::can_cast(from.kind()).then_some(Self(from))
            }

            fn syntax(&self) -> &syntax::Node {
                &self.0
            }
        }

        impl $name {
            pub const KIND: syntax::Kind = $kind;
        }
    };
    (
        #[from($($variant:ident),* $(,)?)]
        $(#[$meta:meta])*
        enum $name:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
             $($variant($variant),)*
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: syntax::Kind) -> bool {
                matches!(kind, $($variant::KIND)|*)
            }

            fn cast(from: syntax::Node) -> Option<Self> {
                Some(match from.kind() {
                    $($variant::KIND => Self::$variant($variant(from)),)*
                    _ => return None,
                })
            }

            fn syntax(&self) -> &syntax::Node {
                match self {
                    $(Self::$variant(this) => &this.0,)*
                }
            }
        }

        $(
            impl From<$variant> for $name {
                fn from(from: $variant) -> Self {
                    Self::$variant(from)
                }
            }

            impl TryFrom<$name> for $variant {
                type Error = ();

                fn try_from(from: $name) -> Result<Self, Self::Error> {
                    match from {
                        $name::$variant(it) => Ok(it),
                        _ => Err(()),
                    }
                }
            }
        )*
    };
}

node! { #[from(NODE_LITERAL)] struct Literal; }
node! { #[from(NODE_APPLY)] struct Apply; }

node! { #[from(Literal, Apply)] enum Expression; }

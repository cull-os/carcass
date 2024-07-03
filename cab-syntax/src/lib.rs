pub mod ast;
mod kind;
mod parser;
mod tokenizer;

pub use tokenizer::{
    Token,
    Tokenizer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {}

impl rowan::Language for Language {
    type Kind = syntax::Kind;

    fn kind_from_raw(from: rowan::SyntaxKind) -> Self::Kind {
        Self::Kind::try_from(from.0).unwrap()
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub mod syntax {
    pub use crate::kind::*;

    pub type Node = rowan::SyntaxNode<crate::Language>;
    pub type Token = rowan::SyntaxToken<crate::Language>;
    pub type Element = rowan::NodeOrToken<Node, Token>;
}

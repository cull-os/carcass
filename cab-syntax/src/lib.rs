mod ast;
mod kind;
mod parse;
mod token;

pub use kind::SyntaxKind::{
    self,
    *,
};
pub use token::{
    Token,
    Tokenizer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {}

impl rowan::Language for Language {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        Self::Kind::try_from(raw.0).unwrap()
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<Language>;
pub type SyntaxToken = rowan::SyntaxToken<Language>;
pub type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

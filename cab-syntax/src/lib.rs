#![feature(const_fn_floating_point_arithmetic, try_trait_v2)]

mod color;
mod kind;

pub use color::*;
pub use kind::*;

pub mod node;
pub mod token;

pub use node::Node;
pub use token::Token;

mod parser;
mod tokenizer;

pub use parser::*;
pub use tokenizer::*;

pub type RowanNode = rowan::SyntaxNode<Language>;
pub type RowanToken = rowan::SyntaxToken<Language>;
pub type RowanElement = rowan::NodeOrToken<RowanNode, RowanToken>;

/// The Cab language type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Language {}

impl rowan::Language for Language {
    type Kind = Kind;

    fn kind_from_raw(from: rowan::SyntaxKind) -> Self::Kind {
        Self::Kind::try_from(from.0).unwrap()
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

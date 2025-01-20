#![feature(gen_blocks, if_let_guard, inline_const_pat, let_chains, try_blocks)]

mod color;
pub use color::*;

pub mod format;

mod kind;
pub use kind::*;

pub mod node;
mod noder;
pub use noder::*;

pub mod token;
mod tokenizer;
pub use tokenizer::*;

pub(crate) type RowanNode = rowan::SyntaxNode<Language>;
pub(crate) type RowanToken = rowan::SyntaxToken<Language>;
pub(crate) type RowanElement = rowan::NodeOrToken<RowanNode, RowanToken>;

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

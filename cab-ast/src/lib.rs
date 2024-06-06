mod ast;
mod kind;
mod parse;

use std::mem::transmute;

pub use ast::*;
pub use kind::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CabLanguage {}

impl rowan::Language for CabLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        let syntax_discriminant = raw.0;
        assert!(syntax_discriminant <= __LAST as u16);

        // SAFETY: We only have a single SyntaxKind enum in this codebase,
        // so there is no chance of mixing them up.
        unsafe { transmute(syntax_discriminant) }
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<CabLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<CabLanguage>;
pub type SyntaxElement = rowan::NodeOrToken<SyntaxNode, SyntaxToken>;

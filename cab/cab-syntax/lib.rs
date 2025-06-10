//! Token, tokenizer, node, noder implementations.

#![feature(
   assert_matches,
   gen_blocks,
   if_let_guard,
   lazy_get,
   lazy_cell_into_inner,
   impl_trait_in_assoc_type,
   let_chains,
   trait_alias
)]

use std::ops;

use enumset::{
   EnumSet,
   enum_set,
};

pub use self::{
   noder::{
      Parse,
      ParseOracle,
      parse_oracle,
   },
   tokenizer::{
      is_valid_plain_identifier,
      tokenize,
   },
};

pub mod node;
mod noder;

pub mod segment;

pub mod token;
mod tokenizer;

#[expect(dead_code)]
mod red {
   use super::*;

   pub type Node = cstree::syntax::ResolvedNode<Kind>;
   pub type ResolvedNode = cstree::syntax::SyntaxNode<Kind>;

   pub type Token = cstree::syntax::ResolvedToken<Kind>;
   pub type ResolvedToken = cstree::syntax::SyntaxToken<Kind>;

   pub type Element = cstree::syntax::ResolvedElement<Kind>;
   pub type ResolvedElement = cstree::syntax::SyntaxElement<Kind>;

   pub type ElementRef<'a> = cstree::util::NodeOrToken<&'a Node, &'a Token>;
   pub type ResolvedElementRef<'a> = cstree::util::NodeOrToken<&'a Node, &'a Token>;
}

pub trait Node = TryFrom<red::Node> + ops::Deref<Target = red::Node>;
pub trait NodeRef<'a> = TryFrom<&'a red::Node> + ops::Deref<Target: ops::Deref<Target = red::Node>>;

pub trait Token = TryFrom<red::Token> + ops::Deref<Target = red::Token>;
pub trait TokenRef<'a> =
   TryFrom<&'a red::Token> + ops::Deref<Target: ops::Deref<Target = red::Token>>;

#[expect(dead_code)]
mod green {
   use std::sync::Arc;

   use super::*;

   pub type Interner = Arc<cstree::interning::MultiThreadedTokenInterner>;

   pub fn interner() -> Interner {
      Arc::new(cstree::interning::new_threaded_interner())
   }

   pub type Checkpoint = cstree::build::Checkpoint;

   pub type NodeBuilder = cstree::build::GreenNodeBuilder<'static, 'static, Kind, Interner>;
   pub type NodeCache = cstree::build::NodeCache<'static, Interner>;

   pub type Node = cstree::green::GreenNode;
   pub type Token = cstree::green::GreenToken;
}

/// The syntax kind.
#[derive(
   derive_more::Display,
   Debug,
   Clone,
   Copy,
   PartialEq,
   Eq,
   Hash,
   enumset::EnumSetType,
   cstree::Syntax,
)]
#[repr(u32)]
#[enumset(no_super_impls)]
#[expect(non_camel_case_types)]
#[non_exhaustive]
pub enum Kind {
   /// Represents any sequence of tokens that was not recognized.
   #[display("an unknown token sequence")]
   TOKEN_ERROR_UNKNOWN,

   /// Anything that matches [`char::is_whitespace`].
   #[display("space")]
   TOKEN_SPACE,

   /// Anything that starts with a `#`.
   ///
   /// When the comment starts with `#` and a nonzero number of `=`, it will be
   /// multiline. Multiline comments can be closed with the initial amount of
   /// `=` and then a `#`, but they don't have to be.
   ///
   /// Comments can be nested. The following is a valid comment:
   ///
   /// ```text
   /// #==
   ///   #=
   ///   ==# (this doesn't close the first comment, nor
   ///        does it close the second comment start
   ///        token, therefore it is ignored)
   ///   =#
   /// ==#
   /// ```
   #[display("a comment")]
   TOKEN_COMMENT,

   #[display("','")]
   #[static_text(",")]
   TOKEN_COMMA,
   #[display("';'")]
   #[static_text(";")]
   TOKEN_SEMICOLON,

   #[display("'<|'")]
   #[static_text("<|")]
   TOKEN_LESS_PIPE,
   #[display("'|>'")]
   #[static_text("|>")]
   TOKEN_PIPE_MORE,

   #[display("'('")]
   #[static_text("(")]
   TOKEN_PARENTHESIS_LEFT,
   #[display("')'")]
   #[static_text(")")]
   TOKEN_PARENTHESIS_RIGHT,

   #[display(r"'\('")]
   TOKEN_INTERPOLATION_START,
   #[display("')'")]
   #[static_text(")")]
   TOKEN_INTERPOLATION_END,

   #[display("'=>'")]
   #[static_text("=>")]
   TOKEN_EQUAL_MORE,

   #[display("':'")]
   #[static_text(":")]
   TOKEN_COLON,
   #[display("'++'")]
   #[static_text("++")]
   TOKEN_PLUS_PLUS,
   #[display("'['")]
   #[static_text("[")]
   TOKEN_BRACKET_LEFT,
   #[display("']'")]
   #[static_text("]")]
   TOKEN_BRACKET_RIGHT,

   #[display("'.'")]
   #[static_text(".")]
   TOKEN_PERIOD,
   #[display("'//'")]
   #[static_text("//")]
   TOKEN_SLASH_SLASH,
   #[display("'{{'")]
   #[static_text("{")]
   TOKEN_CURLYBRACE_LEFT,
   #[display("'}}'")]
   #[static_text("}")]
   TOKEN_CURLYBRACE_RIGHT,

   #[display("'<='")]
   #[static_text("<=")]
   TOKEN_LESS_EQUAL,
   #[display("'<'")]
   #[static_text("<")]
   TOKEN_LESS,
   #[display("'>='")]
   #[static_text(">=")]
   TOKEN_MORE_EQUAL,
   #[display("'>'")]
   #[static_text(">")]
   TOKEN_MORE,

   #[display("'!='")]
   #[static_text("!=")]
   TOKEN_EXCLAMATION_EQUAL,
   #[display("'='")]
   #[static_text("=")]
   TOKEN_EQUAL,

   #[display("'&&'")]
   #[static_text("&&")]
   TOKEN_AMPERSAND_AMPERSAND,
   #[display("'||'")]
   #[static_text("||")]
   TOKEN_PIPE_PIPE,
   #[display("'!'")]
   #[static_text("!")]
   TOKEN_EXCLAMATION,
   #[display("'->'")]
   #[static_text("->")]
   TOKEN_MINUS_MORE,

   #[display("'&'")]
   #[static_text("&")]
   TOKEN_AMPERSAND,
   #[display("'|'")]
   #[static_text("|")]
   TOKEN_PIPE,

   #[display("'+'")]
   #[static_text("+")]
   TOKEN_PLUS,
   #[display("'-'")]
   #[static_text("-")]
   TOKEN_MINUS,
   #[display("'*'")]
   #[static_text("*")]
   TOKEN_ASTERISK,
   #[display("'^'")]
   #[static_text("^")]
   TOKEN_CARET,
   #[display("'/'")]
   #[static_text("/")]
   TOKEN_SLASH,

   #[display("a non-decimal number with no digits")]
   TOKEN_ERROR_NUMBER_NO_DIGIT,
   #[display("an integer")]
   TOKEN_INTEGER,
   #[display("a float")]
   TOKEN_FLOAT,
   #[display("a float with a missing exponent")]
   TOKEN_ERROR_FLOAT_NO_EXPONENT,

   #[display("the keyword 'if'")]
   #[static_text("if")]
   TOKEN_KEYWORD_IF,
   #[display("the keyword 'then'")]
   #[static_text("then")]
   TOKEN_KEYWORD_THEN,
   #[display("the keyword 'else'")]
   #[static_text("else")]
   TOKEN_KEYWORD_ELSE,

   /// See [`NODE_STRING`].
   #[display("content")]
   TOKEN_CONTENT,

   /// The start of a path root type.
   #[display("a root")]
   #[static_text("<")]
   TOKEN_PATH_ROOT_TYPE_START,
   /// The end of a path root type.
   #[display("the closing delimiter of a root")]
   TOKEN_PATH_ROOT_TYPE_END,

   /// A zero width token for the start of a path. Has no content.
   #[display("a path")]
   #[static_text("")]
   TOKEN_PATH_CONTENT_START,
   /// A zero width token for the end of a path. Has no content.
   #[display("the closing delimiter of a path")]
   #[static_text("")]
   TOKEN_PATH_END,

   #[display("'@'")]
   #[static_text("@")]
   TOKEN_AT,

   /// A normal non-quoted identifier. All characters must be either
   /// [`char::is_alphanumeric`], `_`, `-` or `'`. The initial character must
   /// not match [`char::is_ascii_digit`].
   #[display("an identifier")]
   TOKEN_IDENTIFIER,

   #[display("an identifier")]
   TOKEN_QUOTED_IDENTIFIER_START,
   #[display("the closing delimiter of an identifier")]
   TOKEN_QUOTED_IDENTIFIER_END,

   #[display("a string")]
   TOKEN_STRING_START,
   #[display("the closing delimiter of a string")]
   TOKEN_STRING_END,

   #[display("a rune")]
   TOKEN_RUNE_START,
   #[display("the closing delimiter of a rune")]
   TOKEN_RUNE_END,

   #[display("{}", unreachable!())]
   NODE_PARSE_ROOT,
   #[display("an erroneous expression")]
   NODE_ERROR,

   #[display("a prefix operation")]
   NODE_PREFIX_OPERATION,
   #[display("an infix operation")]
   NODE_INFIX_OPERATION,
   #[display("a suffix operation")]
   NODE_SUFFIX_OPERATION,

   #[display("a parenthesized expression")]
   NODE_PARENTHESIS,

   #[display("a list")]
   NODE_LIST,

   #[display("attributes")]
   NODE_ATTRIBUTES,

   /// A node which starts with a [`TOKEN_INTERPOLATION_START`], ends with a
   /// [`TOKEN_INTERPOLATION_END`] while having a node at the middle that can
   /// be cast to an [Expression](crate::node::Expression).
   #[display("{}", unreachable!())]
   NODE_INTERPOLATION,

   #[display("a path")]
   NODE_PATH,
   #[display("{}", unreachable!())]
   NODE_PATH_ROOT,
   #[display("{}", unreachable!())]
   NODE_PATH_ROOT_TYPE,
   #[display("{}", unreachable!())]
   NODE_PATH_CONTENT,

   /// A node that starts with a [`TOKEN_AT`] and has a [`NODE_IDENTIFIER`] as
   /// a child, used for binding expressions to identifiers.
   #[display("a bind")]
   NODE_BIND,

   /// A stringlike that is delimited by a single backtick. See [`NODE_STRING`]
   /// for the definition of stringlike.
   #[display("an identifier")]
   NODE_IDENTIFIER,

   /// A stringlike that is delimited by a single `"` and any number of `=`:
   ///
   /// ```text
   /// "== foo =="
   /// ```
   ///
   /// A stringlike is a sequence of nodes and tokens, where all the immediate
   /// children tokens are start, end or [`TOKEN_CONTENT`]s, while all the
   /// immediate children nodes are all [`NODE_INTERPOLATION`]s.
   #[display("a string")]
   NODE_STRING,

   /// A stringlike that can only contain a single character delimited by `'`.
   /// See [`NODE_STRING`] for the definition of stringlike.
   #[display("a rune")]
   NODE_RUNE,

   #[display("an integer")]
   NODE_INTEGER,
   #[display("a float")]
   NODE_FLOAT,

   #[display("an if")]
   NODE_IF,
}

use Kind::*;

impl Kind {
   /// An enumset of all valid expression starter token kinds.
   pub const EXPRESSIONS: EnumSet<Kind> = enum_set!(
      TOKEN_PARENTHESIS_LEFT
         | TOKEN_BRACKET_LEFT
         | TOKEN_CURLYBRACE_LEFT
         | TOKEN_INTEGER
         | TOKEN_FLOAT
         | TOKEN_KEYWORD_IF
         | TOKEN_PATH_CONTENT_START
         | TOKEN_AT
         | TOKEN_IDENTIFIER
         | TOKEN_QUOTED_IDENTIFIER_START
         | TOKEN_STRING_START
         | TOKEN_RUNE_START
   );

   /// An enumset of all identifier starter token kinds.
   pub const IDENTIFIERS: EnumSet<Kind> =
      enum_set!(TOKEN_IDENTIFIER | TOKEN_QUOTED_IDENTIFIER_START);

   /// Whether if this token can be used as a lambda argument.
   ///
   /// ```txt
   /// max 42 (38) + 61
   ///     t  t    f
   /// ```
   #[must_use]
   pub fn is_argument(self) -> bool {
      let mut arguments = Self::EXPRESSIONS;
      arguments.remove(TOKEN_KEYWORD_IF);

      arguments.contains(self) || self.is_error() // Error nodes are expressions.
   }

   /// Whether if the token should be ignored by the noder.
   #[must_use]
   pub fn is_trivia(self) -> bool {
      matches!(self, TOKEN_COMMENT | TOKEN_SPACE)
   }

   /// Whether if this token is erroneous.
   #[must_use]
   pub fn is_error(self) -> bool {
      matches!(
         self,
         TOKEN_ERROR_UNKNOWN | TOKEN_ERROR_NUMBER_NO_DIGIT | TOKEN_ERROR_FLOAT_NO_EXPONENT
      )
   }

   /// Returns the node and closing kinds of this starting delimiter.
   #[must_use]
   pub fn into_node_and_closing(self) -> Option<(Kind, Kind)> {
      Some(match self {
         TOKEN_PATH_CONTENT_START => (NODE_PATH_CONTENT, TOKEN_PATH_END),
         TOKEN_PATH_ROOT_TYPE_START => (NODE_PATH_ROOT_TYPE, TOKEN_PATH_ROOT_TYPE_END),
         TOKEN_QUOTED_IDENTIFIER_START => (NODE_IDENTIFIER, TOKEN_QUOTED_IDENTIFIER_END),
         TOKEN_STRING_START => (NODE_STRING, TOKEN_STRING_END),
         TOKEN_RUNE_START => (NODE_RUNE, TOKEN_RUNE_END),
         _ => return None,
      })
   }
}

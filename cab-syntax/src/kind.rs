use Kind::*;
use derive_more::Display;
use enumset::{
    EnumSet,
    enum_set,
};

/// derive_more causes [`unreachable`] to warn too many times
/// so we're just suppressing them like this. No, #[allow(unreachable_code)]
/// doesn't suppress the ones coming from the #[derive(Display)].
fn reachable_unreachable() -> &'static str {
    unreachable!()
}

/// The syntax kind.
#[derive(
    Display, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, enumset::EnumSetType, num_enum::TryFromPrimitive,
)]
#[enumset(no_super_impls)]
#[repr(u16)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum Kind {
    /// Represents any sequence of tokens that was not recognized.
    #[display("an unknown token sequence")]
    TOKEN_ERROR_UNKNOWN,

    /// Anything that matches [`char::is_whitespace`].
    #[display("whitespace")]
    TOKEN_WHITESPACE,

    /// Anything that starts with a `#`.
    ///
    /// When the comment starts with `#` and a nonzer number of `=`, it will be
    /// multiline. Multiline comments can be closed with the initial amount of
    /// `=` and then a `#`, but they don't have to be.
    #[display("a comment")]
    TOKEN_COMMENT,

    #[display("';'")]
    TOKEN_SEMICOLON,
    #[display("'?'")]
    TOKEN_QUESTIONMARK,

    #[display("'<|'")]
    TOKEN_LESS_PIPE,
    #[display("'|>'")]
    TOKEN_PIPE_MORE,

    #[display("'('")]
    TOKEN_LEFT_PARENTHESIS,
    #[display("')'")]
    TOKEN_RIGHT_PARENTHESIS,

    #[display(r"'\('")]
    TOKEN_INTERPOLATION_START,
    #[display("')'")]
    TOKEN_INTERPOLATION_END,

    #[display("'@'")]
    TOKEN_AT,
    #[display("'=>'")]
    TOKEN_EQUAL_GREATER,
    #[display("':='")]
    TOKEN_COLON_EQUAL,
    #[display("','")]
    TOKEN_COMMA,

    #[display("':'")]
    TOKEN_COLON,
    #[display("'++'")]
    TOKEN_PLUS_PLUS,
    #[display("'['")]
    TOKEN_LEFT_BRACKET,
    #[display("']'")]
    TOKEN_RIGHT_BRACKET,

    #[display("'//'")]
    TOKEN_SLASH_SLASH,
    #[display("'.'")]
    TOKEN_PERIOD,
    #[display("'{{'")]
    TOKEN_LEFT_CURLYBRACE,
    #[display("'}}'")]
    TOKEN_RIGHT_CURLYBRACE,

    #[display("'!='")]
    TOKEN_EXCLAMATION_EQUAL,
    #[display("'=='")]
    TOKEN_EQUAL_EQUAL,
    #[display("'<='")]
    TOKEN_LESS_EQUAL,
    #[display("'<'")]
    TOKEN_LESS,
    #[display("'>='")]
    TOKEN_MORE_EQUAL,
    #[display("'>'")]
    TOKEN_MORE,

    #[display("'&&'")]
    TOKEN_AMPERSAND_AMPERSAND,
    #[display("'||'")]
    TOKEN_PIPE_PIPE,
    #[display("'!'")]
    TOKEN_EXCLAMATIONMARK,
    #[display("'->'")]
    TOKEN_MINUS_MORE,

    #[display("'&'")]
    TOKEN_AMPERSAND,
    #[display("'|'")]
    TOKEN_PIPE,

    #[display("'+'")]
    TOKEN_PLUS,
    #[display("'-'")]
    TOKEN_MINUS,
    #[display("'*'")]
    TOKEN_ASTERISK,
    #[display("'^'")]
    TOKEN_CARET,
    #[display("'/'")]
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
    TOKEN_LITERAL_IF,
    #[display("the keyword 'then'")]
    TOKEN_LITERAL_THEN,
    #[display("the keyword 'is'")]
    TOKEN_LITERAL_IS,
    #[display("the keyword 'else'")]
    TOKEN_LITERAL_ELSE,

    /// A path. Valid paths start with `./`, `..` or `/`, followed by
    /// characters that are either [alphanumeric](char::is_alphanumeric) or
    /// are any of the following characters: `.`, `/`, `_`, `-`, `\`.
    ///
    /// The `\` character can be used to escape characters that are normally
    /// not allowed in paths, like spaces and other weird characters.
    ///
    /// Every path part will be represented using this kind, so a path node with
    /// interpolation will be represented as the following:
    ///
    /// ```txt
    /// ./foo\(bar)baz -- TOKEN_PATH
    /// +---/\|\|/\-- TOKEN_INTERPOLATION_END
    /// |     | +-- TOKEN_IDENTIFIER
    /// |     +-- TOKEN_INTERPOLATION_START
    /// +-- TOKEN_PATH
    /// ```
    #[display("a path")]
    TOKEN_PATH,

    #[display("content")]
    TOKEN_CONTENT,

    /// A normal non-quoted identifier. The initial character must not match
    /// [`char::is_ascii_digit`], the other characters must be either
    /// [`char::is_alphanumeric`], `_`, `-` or `'`.
    #[display("an identifier")]
    TOKEN_IDENTIFIER,

    #[display("an identifier")]
    TOKEN_IDENTIFIER_START,
    #[display("the closing delimiter of an identifier")]
    TOKEN_IDENTIFIER_END,

    #[display("a string")]
    TOKEN_STRING_START,
    #[display("the closing delimiter of a string")]
    TOKEN_STRING_END,

    #[display("a rune")]
    TOKEN_RUNE_START,
    #[display("the closing delimiter of a rune")]
    TOKEN_RUNE_END,

    #[display("an island")]
    TOKEN_ISLAND_START,
    #[display("the closing delimiter of an island")]
    TOKEN_ISLAND_END,

    #[display("{}", reachable_unreachable())]
    NODE_ROOT,
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

    #[display("an attribute list")]
    NODE_ATTRIBUTE_LIST,

    /// A node which starts with a [`TOKEN_INTERPOLATION_START`], ends with a
    /// [`TOKEN_INTERPOLATION_END`] while having a node at the middle that can
    /// be cast to an [Expression](crate::node::Expression).
    #[display("{}", reachable_unreachable())]
    NODE_INTERPOLATION,

    /// A node that only has [`TOKEN_PATH`]s and [`NODE_INTERPOLATION`]s as its
    /// direct children without any delimiters.
    #[display("a path")]
    NODE_PATH,

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
    /// children tokens are [`TOKEN_CONTENT`]s, while all the immediate children
    /// nodes are all [`NODE_INTERPOLATION`]s.
    #[display("a string")]
    NODE_STRING,

    #[display("a rune")]
    NODE_RUNE,

    /// A stringlike that is delimited by `<` and `>`. See [`NODE_STRING`] for
    /// the definition of stringlike.
    #[display("a number")]
    NODE_ISLAND,

    /// A node containing a single token, which can be either a
    /// [`TOKEN_INTEGER`] or [`TOKEN_FLOAT`].
    #[display("a number")]
    NODE_NUMBER,

    #[display("an if then")]
    NODE_IF_THEN,
    #[display("an if is")]
    NODE_IF_IS,
}

impl From<Kind> for rowan::SyntaxKind {
    fn from(kind: Kind) -> Self {
        Self(kind as u16)
    }
}

impl Kind {
    pub const EXPRESSION_SET: EnumSet<Kind> = enum_set!(
        TOKEN_LEFT_PARENTHESIS
            | TOKEN_LEFT_BRACKET
            | TOKEN_LEFT_CURLYBRACE
            | TOKEN_INTEGER
            | TOKEN_FLOAT
            | TOKEN_LITERAL_IF
            | TOKEN_PATH
            | TOKEN_IDENTIFIER
            | TOKEN_IDENTIFIER_START
            | TOKEN_STRING_START
            | TOKEN_RUNE_START
            | TOKEN_ISLAND_START
    );
    pub const IDENTIFIER_SET: EnumSet<Kind> = enum_set!(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START);

    /// Whether if this token can be used as a lambda argument.
    ///
    /// ```txt
    /// max 42 (38) + 61
    ///     t  t    f
    /// ```
    pub fn is_argument(self) -> bool {
        Self::EXPRESSION_SET.difference(TOKEN_LITERAL_IF.into()).contains(self) || self.is_error() // Error nodes are expressions.
    }

    /// Whether if the token should be ignored by the noder.
    pub fn is_trivia(self) -> bool {
        matches!(self, TOKEN_COMMENT | TOKEN_WHITESPACE)
    }

    /// Whether if this token is erroneous.
    pub fn is_error(self) -> bool {
        matches!(
            self,
            TOKEN_ERROR_UNKNOWN | TOKEN_ERROR_NUMBER_NO_DIGIT | TOKEN_ERROR_FLOAT_NO_EXPONENT
        )
    }

    /// Returns the node and closing kinds of this kind.
    pub fn as_node_and_closing(self) -> Option<(Kind, Kind)> {
        Some(match self {
            TOKEN_IDENTIFIER_START => (NODE_IDENTIFIER, TOKEN_IDENTIFIER_END),
            TOKEN_STRING_START => (NODE_STRING, TOKEN_STRING_END),
            TOKEN_RUNE_START => (NODE_RUNE, TOKEN_RUNE_END),
            TOKEN_ISLAND_START => (NODE_ISLAND, TOKEN_ISLAND_END),
            _ => return None,
        })
    }
}

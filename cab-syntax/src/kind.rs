use derive_more::Display;

/// The Cab syntax kind.
#[derive(
    Display,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    enumset::EnumSetType,
    num_enum::TryFromPrimitive,
)]
#[enumset(no_super_impls)]
#[repr(u16)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum Kind {
    /// Represents any sequence of tokens that was not recognized.
    #[display(fmt = "an unknown token sequence")]
    TOKEN_ERROR,

    /// Anything that matches [`char::is_whitespace`].
    #[display(fmt = "whitespace")]
    TOKEN_WHITESPACE,

    /// Anything that starts with a `#`.
    ///
    /// When the comment starts with more than 3 `#` characters, it will be
    /// multiline. Multiline comments can be closed with the initial delimiter,
    /// but they don't have to be.
    #[display(fmt = "a comment")]
    TOKEN_COMMENT, // #[^\r\n]* and (#{3,}).*\1

    #[display(fmt = "'$'")]
    TOKEN_DOLLAR,
    TOKEN_PIPE_MORE,

    #[display(fmt = "'('")]
    TOKEN_LEFT_PARENTHESIS,
    #[display(fmt = "')'")]
    TOKEN_RIGHT_PARENTHESIS,

    #[display(fmt = "'++'")]
    TOKEN_PLUS_PLUS,
    #[display(fmt = "'['")]
    TOKEN_LEFT_BRACKET,
    #[display(fmt = "']'")]
    TOKEN_RIGHT_BRACKET,

    #[display(fmt = "'==>'")]
    TOKEN_EQUAL_EQUAL_MORE,
    #[display(fmt = "'<=='")]
    TOKEN_LESS_EQUAL_EQUAL,
    #[display(fmt = "'//'")]
    TOKEN_SLASH_SLASH,
    #[display(fmt = "'.'")]
    TOKEN_PERIOD,
    #[display(fmt = "'{{'")]
    TOKEN_LEFT_CURLYBRACE,
    #[display(fmt = "'}}'")]
    TOKEN_RIGHT_CURLYBRACE,
    #[display(fmt = "'?'")]
    TOKEN_QUESTIONMARK,
    #[display(fmt = "';'")]
    TOKEN_SEMICOLON,

    #[display(fmt = "'='")]
    TOKEN_EQUAL,
    #[display(fmt = "'=='")]
    TOKEN_EQUAL_EQUAL,
    #[display(fmt = "'!='")]
    TOKEN_EXCLAMATION_EQUAL,
    #[display(fmt = "'<='")]
    TOKEN_LESS_EQUAL,
    #[display(fmt = "'<'")]
    TOKEN_LESS,
    #[display(fmt = "'>='")]
    TOKEN_MORE_EQUAL,
    #[display(fmt = "'>'")]
    TOKEN_MORE,
    #[display(fmt = "'->'")]
    TOKEN_MINUS_MORE,

    #[display(fmt = "'@'")]
    TOKEN_AT,
    #[display(fmt = "','")]
    TOKEN_COMMA,
    #[display(fmt = "':'")]
    TOKEN_COLON,

    #[display(fmt = "'+'")]
    TOKEN_PLUS,
    #[display(fmt = "'-'")]
    TOKEN_MINUS,
    #[display(fmt = "'*'")]
    TOKEN_ASTERISK,
    #[display(fmt = "'**'")]
    TOKEN_ASTERISK_ASTERISK,
    #[display(fmt = "'/'")]
    TOKEN_SLASH,

    #[display(fmt = "an integer")]
    TOKEN_INTEGER,
    #[display(fmt = "a float")]
    TOKEN_FLOAT,

    #[display(fmt = "the keyword 'if'")]
    TOKEN_LITERAL_IF,
    #[display(fmt = "the keyword 'then'")]
    TOKEN_LITERAL_THEN,
    #[display(fmt = "the keyword 'else'")]
    TOKEN_LITERAL_ELSE,

    #[display(fmt = "the keyword 'and'")]
    TOKEN_LITERAL_AND,
    #[display(fmt = "the keyword 'or'")]
    TOKEN_LITERAL_OR,
    #[display(fmt = "the keyword 'not'")]
    TOKEN_LITERAL_NOT,

    #[display(fmt = "${{")]
    TOKEN_INTERPOLATION_START,
    #[display(fmt = "}}")]
    TOKEN_INTERPOLATION_END,

    /// A path. Valid paths start with `./`, `..` or `/`, followed by
    /// characters that are either [alphanumeric](char::is_alphanumeric) or
    /// are any of the following characters: `.`, `/`, `_`, `-`, `\`.
    ///
    /// The `\` character can be used to escape characters that are normally
    /// not allowed in paths, like spaces and other weird characters.
    /// It is also useful to escape `${` literally, to not begin string
    /// interpolation like so: `./\$\{foo`
    ///
    /// Every path part will be represented using this kind, so a path node with
    /// interpolation will be represented as the following:
    ///
    /// ```txt
    /// ./foo${bar}baz -- TOKEN_PATH
    /// +---/\|\|/\-- TOKEN_INTERPOLATION_END
    /// |     | +-- TOKEN_IDENTIFIER
    /// |     +-- TOKEN_INTERPOLATION_START
    /// +-- TOKEN_PATH
    /// ```
    #[display(fmt = "a path")]
    TOKEN_PATH,

    #[display(fmt = "the content of a string-like")]
    TOKEN_CONTENT,

    /// A normal non-quoted identifier. The initial character must not match
    /// [`char::is_ascii_digit`], the other characters must be either
    /// [`char::is_alphanumeric`], `_` or `-`.
    #[display(fmt = "an identifier")]
    TOKEN_IDENTIFIER,

    #[display(fmt = "an identifier")]
    TOKEN_IDENTIFIER_START,
    #[display(fmt = "the end of an identifier")]
    TOKEN_IDENTIFIER_END,

    #[display(fmt = "a string")]
    TOKEN_STRING_START,
    #[display(fmt = "the end of a string")]
    TOKEN_STRING_END,

    #[display(fmt = "an island")]
    TOKEN_ISLAND_START,
    #[display(fmt = "the end of an island")]
    TOKEN_ISLAND_END,

    NODE_ROOT,
    NODE_ERROR,

    NODE_PARENTHESIS, // (<expression>)

    NODE_LIST, // [<expression>*]

    NODE_ATTRIBUTE_SET,     // { <attribute | attribute-inherit>* }
    NODE_ATTRIBUTE,         // <attribute-path> = <expression>;
    NODE_ATTRIBUTE_PATH,    // <identifier><.<identifier>>*
    NODE_ATTRIBUTE_INHERIT, // <identifier>;

    NODE_ATTRIBUTE_SELECT, // <expression>.<identifier>
    NODE_ATTRIBUTE_CHECK,  // <expression> ? <attribute-path>

    NODE_BIND, // <identifier> @

    NODE_USE, // <bind>? <expression> ==> <expression>

    NODE_LAMBDA, // <identifier | lambda-parameter-pattern>: <expression>
    NODE_LAMBDA_PARAMETER_IDENTIFIER, // <identifier>
    NODE_LAMBDA_PARAMETER_PATTERN, // <bind>? { <<attribute>,>* }
    NODE_LAMBDA_PARAMETER_PATTERN_ATTRIBUTE, // <identifier> <? <expression>>?

    NODE_APPLICATION, // <expression> <expression>

    NODE_PREFIX_OPERATION, // <operator> <expression>
    NODE_INFIX_OPERATION,  // <expression> <operator> <expression>

    /// A node which starts with a [`TOKEN_INTERPOLATION_START`], ends with a
    /// [`TOKEN_INTERPOLATION_END`] while having a node at the middle that can
    /// be cast to an [Expression](crate::node::Expression)
    NODE_INTERPOLATION,

    /// A node that only has [`TOKEN_PATH`]s and [`NODE_INTERPOLATION`]s as its
    /// direct children without any delimiters.
    NODE_PATH,

    /// A stringlike that is delimited by a single backtick. See [`NODE_STRING`]
    /// for the definition of stringlike.
    NODE_IDENTIFIER,

    /// A stringlike that is delimited by a single `"` or any number of `'`.
    ///
    /// A stringlike is a sequence of nodes and tokens, where all the immediate
    /// children tokens are [`TOKEN_CONTENT`]s, while all the immediate children
    /// nodes are all [`NODE_INTERPOLATION`]s.
    NODE_STRING,

    /// A stringlike that is delimited by `<` and `>`. See [`NODE_STRING`] for
    /// the definition of stringlike.
    NODE_ISLAND,

    /// A node containing a single token, which can be either a
    /// [`TOKEN_INTEGER`] or [`TOKEN_FLOAT`].
    NODE_NUMBER,

    NODE_IF_ELSE, // if <expression> then <expression> else <expression>
}

use Kind::*;

impl From<Kind> for rowan::SyntaxKind {
    fn from(kind: Kind) -> Self {
        Self(kind as u16)
    }
}

impl Kind {
    /// Whether if this token is a literal, such as a float or integer.
    pub fn is_literal(self) -> bool {
        matches!(self, TOKEN_FLOAT | TOKEN_INTEGER)
    }

    /// Whether if this token can be used as a function argument.
    ///
    /// ```txt
    /// max 42 (38) + 61
    ///     t  t    f
    /// ```
    pub fn is_argument(self) -> bool {
        match self {
            TOKEN_LEFT_PARENTHESIS
            | TOKEN_LEFT_BRACKET
            | TOKEN_LEFT_CURLYBRACE
            | TOKEN_IDENTIFIER
            | TOKEN_IDENTIFIER_START
            | TOKEN_STRING_START
            | TOKEN_PATH
            | TOKEN_ISLAND_START => true,
            _ => self.is_literal(),
        }
    }

    /// Whether if the token should be ignored by the parser.
    pub fn is_trivia(self) -> bool {
        matches!(self, TOKEN_COMMENT | TOKEN_WHITESPACE)
    }
}

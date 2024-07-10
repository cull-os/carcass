use std::marker;

/// The Cab syntax kind.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    num_enum::TryFromPrimitive,
    marker::ConstParamTy,
)]
#[repr(u16)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum Kind {
    TOKEN_ERROR,

    TOKEN_WHITESPACE,
    TOKEN_COMMENT, // #[^\r\n]* and (#{3,}).*\1

    TOKEN_DOLLAR,    // $
    TOKEN_PIPE_MORE, // |>

    TOKEN_LEFT_PARENTHESIS,  // (
    TOKEN_RIGHT_PARENTHESIS, // )

    TOKEN_PLUS_PLUS,     // ++
    TOKEN_LEFT_BRACKET,  // [
    TOKEN_RIGHT_BRACKET, // ]

    TOKEN_EQUAL_EQUAL_MORE, // ==>
    TOKEN_LESS_EQUAL_EQUAL, // <==
    TOKEN_SLASH_SLASH,      // //
    TOKEN_PERIOD,           // .
    TOKEN_LEFT_CURLYBRACE,  // {
    TOKEN_RIGHT_CURLYBRACE, // }
    TOKEN_QUESTIONMARK,     // ?
    TOKEN_SEMICOLON,        // ;

    TOKEN_EQUAL,             // =
    TOKEN_EQUAL_EQUAL,       // ==
    TOKEN_EXCLAMATION_EQUAL, // !=
    TOKEN_LESS_EQUAL,        // <=
    TOKEN_LESS,              // <
    TOKEN_MORE_EQUAL,        // >=
    TOKEN_MORE,              // >
    TOKEN_MINUS_MORE,        // ->

    TOKEN_AT,    // @
    TOKEN_COMMA, // ,
    TOKEN_COLON, // :

    TOKEN_PLUS,              // +
    TOKEN_MINUS,             // -
    TOKEN_ASTERISK,          // *
    TOKEN_ASTERISK_ASTERISK, // **
    TOKEN_SLASH,             // /

    TOKEN_INTEGER, // 38
    TOKEN_FLOAT,   // 3.14

    TOKEN_LITERAL_IF,   // if
    TOKEN_LITERAL_THEN, // then
    TOKEN_LITERAL_ELSE, // else

    TOKEN_LITERAL_AND, // and
    TOKEN_LITERAL_OR,  // or
    TOKEN_LITERAL_NOT, // not

    TOKEN_INTERPOLATION_START, // ${
    TOKEN_INTERPOLATION_END,   // }

    // /etc/resolv.conf, ./wallpaper.png, ./foo${bar}
    TOKEN_PATH,

    TOKEN_CONTENT,

    TOKEN_IDENTIFIER, // fooBar, foo-bar, foo_bar

    // `foo bar baz`, `?? lmao ${baz}`
    TOKEN_IDENTIFIER_START,
    TOKEN_IDENTIFIER_END,

    // "foo\n\t${bar}", ''foo bar'', ''''' asdasd "hey '' asdads ''' asddklfjskaj'''''
    TOKEN_STRING_START,
    TOKEN_STRING_END,

    // <github:${user}/${repo}>
    TOKEN_ISLAND_START,
    TOKEN_ISLAND_END,

    NODE_ROOT,
    NODE_ERROR,

    NODE_PARENTHESIS, // (<expression>)

    NODE_LIST, // [<expression>*]

    NODE_ATTRIBUTE_SET,     // { <attribute | attribute-inherit>* }
    NODE_ATTRIBUTE_ENTRY,   // <attribute-path> = <attribute-value>;
    NODE_ATTRIBUTE_PATH,    // <identifier><.<identifier>>*
    NODE_ATTRIBUTE_INHERIT, // <identifier>;

    NODE_ATTRIBUTE_SELECT, // <expression>.<identifier>
    NODE_ATTRIBUTE_CHECK,  // <expression> ? <attribute-path>

    NODE_BIND, // <identifier> @

    NODE_USE, // <bind>? <expression> ==> <expression>

    NODE_LAMBDA, // <identifier | lambda-parameter-pattern>: <expression>
    NODE_LAMBDA_PARAMETER_IDENTIFIER, // <identifier>
    NODE_LAMBDA_PARAMETER_PATTERN, // <bind>? { <<entry>,>* }
    NODE_LAMBDA_PARAMETER_PATTERN_ENTRY, // <identifier> <? <expression>>?

    NODE_APPLICATION, // <expression> <expression>

    NODE_PREFIX_OPERATION, // <operator> <expression>
    NODE_INFIX_OPERATION,  // <expression> <operator> <expression>

    NODE_INTERPOLATION, // ${<expression>}

    NODE_PATH,       // <path-content | interpolation>*
    NODE_IDENTIFIER, // <identifier-start><identifier-content | interpolation>*<identifier-end>
    NODE_STRING,     // <string-start><string-content | interpolation>*<string-end>
    NODE_ISLAND,     // <island-start><island-content | interpolation>*<island-end>

    NODE_NUMBER, // 42, 3.14

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
    /// # Example
    ///
    /// ```ignore
    /// max 42 38 + 61
    ///     +  +  -
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

use num_enum::TryFromPrimitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    // TOKEN
    TOKEN_ERROR,
    TOKEN_COMMENT,    // #<anything until end of line>
    TOKEN_WHITESPACE, // \n, \t

    TOKEN_LEFT_PARENTHESIS,  // (
    TOKEN_RIGHT_PARENTHESIS, // )

    TOKEN_PLUS_PLUS,     // ++
    TOKEN_LEFT_BRACKET,  // [
    TOKEN_RIGHT_BRACKET, // ]

    TOKEN_EQUAL_MORE,       // =>
    TOKEN_SLASH_SLASH,      // //
    TOKEN_LEFT_CURLYBRACE,  // {
    TOKEN_RIGHT_CURLYBRACE, // }

    TOKEN_LITERAL_IF,   // if
    TOKEN_LITERAL_THEN, // then
    TOKEN_LITERAL_ELSE, // else

    TOKEN_EQUAL,             // =
    TOKEN_EXCLAMATION_EQUAL, // !=
    TOKEN_LESS,              // <
    TOKEN_LESS_EQUAL,        // <=
    TOKEN_MORE,              // >
    TOKEN_MORE_EQUAL,        // >=
    TOKEN_HYPHEN_GREATER,    // ->
    TOKEN_LITERAL_AND,       // and
    TOKEN_LITERAL_OR,        // or
    TOKEN_LITERAL_NOT,       // not

    TOKEN_PERIOD,    // .
    TOKEN_COLON,     // :
    TOKEN_SEMICOLON, // ;

    TOKEN_PLUS,     // +
    TOKEN_MINUS,    // -
    TOKEN_ASTERISK, // *
    TOKEN_SLASH,    // /

    TOKEN_INTEGER, // 38
    TOKEN_FLOAT,   // 3.14

    TOKEN_INTERPOL_START, // ${
    TOKEN_INTERPOL_END,   // }

    TOKEN_IDENTIFIER, // fooBar, foo-bar, foo_bar, `foo bar baz`, `?? lmao`

    TOKEN_STRING_START,   // "
    TOKEN_STRING_CONTENT, // foo\n\t\r\u0123
    TOKEN_STRING_END,     // "

    TOKEN_PATH, // /etc/resolv.conf, ./wallpaper.png

    // AST
    NODE_ERROR,
    NODE_ROOT,

    NODE_LITERAL, // 1, "foo", 3.14

    NODE_IDENTIFIER, // <identifier>

    NODE_APPLY, // <expression> <expression>

    NODE_SELECT,          // <expression>.<identifier | integer>
    NODE_CHECK_ATTRIBUTE, // <expression> ? <identifier | integer><.<identifier | integer>>*

    NODE_LIST,

    NODE_ATTRIBUTE_SET,   // { <attribute-path>* }
    NODE_ATTRIBUTE,       // <identifier><.<identifier>>* = <expression>;
    NODE_ATTRIBUTE_PATH,  // <identifier><.<identifier>>*
    NODE_ATTRIBUTE_VALUE, // <expression>

    NODE_INTERPOLATION, // ${baz}
    NODE_STRING,        // "foo bar ${baz}"
    NODE_PATH,          // /foo/${bar}, ./asd/${def}

    NODE_IF_ELSE, // if <expression> then <expression> else <expression>

    NODE_LAMBDA,                         // <identifier | bind>: <expression>
    NODE_LAMBDA_PARAMETER_IDENTIFIER,    // <identifier>
    NODE_LAMBDA_PARAMETER_PATTERN,       // <identifier> @ { <<entry>,>* }
    NODE_LAMBDA_PARAMETER_PATTERN_BIND,  // <identifier> @
    NODE_LAMBDA_PARAMETER_PATTERN_ENTRY, // <identifier> <? <expr>>?

    NODE_PARENTHESIS, // (<expression>)

    NODE_UNARY_OPERATION,  // <operator> <expression>
    NODE_BINARY_OPERATION, // <expression> <operator> <expression>

    #[doc(hidden)]
    __LAST,
}

pub use SyntaxKind::*;

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl SyntaxKind {
    /// Whether if this token is a literal, such as a float, integer, or path.
    pub fn is_literal(self) -> bool {
        matches!(self, TOKEN_FLOAT | TOKEN_INTEGER | TOKEN_PATH)
    }

    /// Whether if this token can be used as a function argument.
    ///
    /// # Example
    ///
    /// ```ignore
    /// max 42 38 + 61
    /// yyy yy yy n
    /// ```
    pub fn is_argument(self) -> bool {
        match self {
            TOKEN_LEFT_PARENTHESIS
            | TOKEN_LEFT_BRACKET
            | TOKEN_LEFT_CURLYBRACE
            | TOKEN_STRING_START
            | TOKEN_IDENTIFIER => true,
            _ => self.is_literal(),
        }
    }

    /// Whether if the token should be ignored by the parser.
    pub fn is_trivia(self) -> bool {
        matches!(self, TOKEN_COMMENT | TOKEN_ERROR | TOKEN_WHITESPACE)
    }
}

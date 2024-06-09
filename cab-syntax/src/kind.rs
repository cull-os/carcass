use colored::CustomColor;
use num_enum::TryFromPrimitive;

#[rustfmt::skip]
pub const SYNTAX_COLORS: &[CustomColor] = &[
    CustomColor { r: 0x00, g: 0x00, b: 0x00 },
    CustomColor { r: 0x2F, g: 0x4F, b: 0x4F },
    CustomColor { r: 0x55, g: 0x6B, b: 0x2F },
    CustomColor { r: 0x8B, g: 0x45, b: 0x13 },
    CustomColor { r: 0x7F, g: 0x00, b: 0x00 },
    CustomColor { r: 0x19, g: 0x19, b: 0x70 },
    CustomColor { r: 0x80, g: 0x80, b: 0x00 },
    CustomColor { r: 0x77, g: 0x88, b: 0x99 },
    CustomColor { r: 0x00, g: 0x80, b: 0x00 },
    CustomColor { r: 0x3C, g: 0xB3, b: 0x71 },
    CustomColor { r: 0xBC, g: 0x8F, b: 0x8F },
    CustomColor { r: 0x66, g: 0x33, b: 0x99 },
    CustomColor { r: 0x00, g: 0x80, b: 0x80 },
    CustomColor { r: 0xB8, g: 0x86, b: 0x0B },
    CustomColor { r: 0x46, g: 0x82, b: 0xB4 },
    CustomColor { r: 0x00, g: 0x00, b: 0x80 },
    CustomColor { r: 0xD2, g: 0x69, b: 0x1E },
    CustomColor { r: 0x9A, g: 0xCD, b: 0x32 },
    CustomColor { r: 0x20, g: 0xB2, b: 0xAA },
    CustomColor { r: 0xCD, g: 0x5C, b: 0x5C },
    CustomColor { r: 0x32, g: 0xCD, b: 0x32 },
    CustomColor { r: 0x8F, g: 0xBC, b: 0x8F },
    CustomColor { r: 0x8B, g: 0x00, b: 0x8B },
    CustomColor { r: 0xB0, g: 0x30, b: 0x60 },
    CustomColor { r: 0x99, g: 0x32, b: 0xCC },
    CustomColor { r: 0xFF, g: 0x00, b: 0x00 },
    CustomColor { r: 0xFF, g: 0xA5, b: 0x00 },
    CustomColor { r: 0xFF, g: 0xD7, b: 0x00 },
    CustomColor { r: 0xFF, g: 0xFF, b: 0x00 },
    CustomColor { r: 0xC7, g: 0x15, b: 0x85 },
    CustomColor { r: 0x00, g: 0x00, b: 0xCD },
    CustomColor { r: 0xDE, g: 0xB8, b: 0x87 },
    CustomColor { r: 0x00, g: 0xFF, b: 0x00 },
    CustomColor { r: 0x00, g: 0xFF, b: 0x7F },
    CustomColor { r: 0x41, g: 0x69, b: 0xE1 },
    CustomColor { r: 0xE9, g: 0x96, b: 0x7A },
    CustomColor { r: 0xDC, g: 0x14, b: 0x3C },
    CustomColor { r: 0x00, g: 0xFF, b: 0xFF },
    CustomColor { r: 0x00, g: 0xBF, b: 0xFF },
    CustomColor { r: 0xF4, g: 0xA4, b: 0x60 },
    CustomColor { r: 0x93, g: 0x70, b: 0xDB },
    CustomColor { r: 0x00, g: 0x00, b: 0xFF },
    CustomColor { r: 0xA0, g: 0x20, b: 0xF0 },
    CustomColor { r: 0xAD, g: 0xFF, b: 0x2F },
    CustomColor { r: 0xFF, g: 0x63, b: 0x47 },
    CustomColor { r: 0xDA, g: 0x70, b: 0xD6 },
    CustomColor { r: 0xD8, g: 0xBF, b: 0xD8 },
    CustomColor { r: 0xFF, g: 0x00, b: 0xFF },
    CustomColor { r: 0xDB, g: 0x70, b: 0x93 },
    CustomColor { r: 0xF0, g: 0xE6, b: 0x8C },
    CustomColor { r: 0x64, g: 0x95, b: 0xED },
    CustomColor { r: 0xDD, g: 0xA0, b: 0xDD },
    CustomColor { r: 0xB0, g: 0xE0, b: 0xE6 },
    CustomColor { r: 0x90, g: 0xEE, b: 0x90 },
    CustomColor { r: 0xFF, g: 0x14, b: 0x93 },
    CustomColor { r: 0x7F, g: 0xFF, b: 0xD4 },
    CustomColor { r: 0xFA, g: 0xFA, b: 0xD2 },
    CustomColor { r: 0xFF, g: 0x69, b: 0xB4 },
    CustomColor { r: 0xFF, g: 0xB6, b: 0xC1 },
    CustomColor { r: 0xF8, g: 0xF8, b: 0xFF },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive)]
#[repr(u16)]
#[allow(non_camel_case_types)]
#[non_exhaustive]
pub enum SyntaxKind {
    // TOKEN
    TOKEN_ERROR,
    TOKEN_WHITESPACE, // \n, \t
    TOKEN_COMMENT,    // #<anything until end of line>

    TOKEN_DOLLAR,       // $
    TOKEN_PIPE_GREATER, // |>

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
    TOKEN_MINUS_GREATER,     // ->

    TOKEN_AT,    // @
    TOKEN_COMMA, // ,
    TOKEN_COLON, // :

    TOKEN_PLUS,     // +
    TOKEN_MINUS,    // -
    TOKEN_ASTERISK, // *
    TOKEN_SLASH,    // /

    TOKEN_INTEGER, // 38
    TOKEN_FLOAT,   // 3.14

    TOKEN_IDENTIFIER, // fooBar, foo-bar, foo_bar

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

    // `foo bar baz`, `?? lmao ${baz}`
    TOKEN_IDENTIFIER_START,
    TOKEN_IDENTIFIER_CONTENT,
    TOKEN_IDENTIFIER_END,

    // "foo\n\t${bar}", ''foo bar'', ''''' asdasd "hey '' asdads ''' asddklfjskaj'''''
    TOKEN_STRING_START,
    TOKEN_STRING_CONTENT,
    TOKEN_STRING_END,

    // <github:${user}/${repo}>
    TOKEN_ISLAND_START,
    TOKEN_ISLAND_CONTENT,
    TOKEN_ISLAND_END,

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
}

pub use SyntaxKind::*;

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl SyntaxKind {
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
    /// yyy yy yy n
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
        matches!(self, TOKEN_COMMENT | TOKEN_ERROR | TOKEN_WHITESPACE)
    }
}

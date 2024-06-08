use colored::CustomColor;
use num_enum::TryFromPrimitive;

#[rustfmt::skip]
pub const SYNTAX_COLORS: &[CustomColor] = &[
    CustomColor { r: 0xFF, g: 0x00, b: 0x00 },
    CustomColor { r: 0x00, g: 0xFF, b: 0x00 },
    CustomColor { r: 0x00, g: 0x00, b: 0xFF },
    CustomColor { r: 0xFF, g: 0xFF, b: 0x00 },
    CustomColor { r: 0x80, g: 0x00, b: 0x80 },
    CustomColor { r: 0xFF, g: 0xA5, b: 0x00 },
    CustomColor { r: 0xFF, g: 0xC0, b: 0xCB },
    CustomColor { r: 0xA5, g: 0x2A, b: 0x2A },
    CustomColor { r: 0x80, g: 0x80, b: 0x80 },
    CustomColor { r: 0x00, g: 0x00, b: 0x00 },
    CustomColor { r: 0xFF, g: 0xFF, b: 0xFF },
    CustomColor { r: 0x00, g: 0xFF, b: 0xFF },
    CustomColor { r: 0xFF, g: 0x00, b: 0xFF },
    CustomColor { r: 0x00, g: 0xFF, b: 0x00 },
    CustomColor { r: 0x80, g: 0x00, b: 0x00 },
    CustomColor { r: 0x00, g: 0x00, b: 0x80 },
    CustomColor { r: 0x80, g: 0x80, b: 0x00 },
    CustomColor { r: 0xC0, g: 0xC0, b: 0xC0 },
    CustomColor { r: 0x00, g: 0x80, b: 0x80 },
    CustomColor { r: 0x00, g: 0xFF, b: 0xFF },
    CustomColor { r: 0xFF, g: 0x00, b: 0xFF },
    CustomColor { r: 0x4B, g: 0x00, b: 0x82 },
    CustomColor { r: 0xDC, g: 0x14, b: 0x3C },
    CustomColor { r: 0x40, g: 0xE0, b: 0xD0 },
    CustomColor { r: 0xE6, g: 0xE6, b: 0xFA },
    CustomColor { r: 0x98, g: 0xFB, b: 0x98 },
    CustomColor { r: 0xFF, g: 0xDA, b: 0xB9 },
    CustomColor { r: 0xDD, g: 0xA0, b: 0xDD },
    CustomColor { r: 0xFA, g: 0x80, b: 0x72 },
    CustomColor { r: 0x7F, g: 0xFF, b: 0x00 },
    CustomColor { r: 0x00, g: 0x47, b: 0xAB },
    CustomColor { r: 0xDA, g: 0xA5, b: 0x20 },
    CustomColor { r: 0x99, g: 0x66, b: 0xCC },
    CustomColor { r: 0x0F, g: 0x52, b: 0xBA },
    CustomColor { r: 0x50, g: 0xC8, b: 0x78 },
    CustomColor { r: 0xE0, g: 0x11, b: 0x5F },
    CustomColor { r: 0xFF, g: 0xC8, b: 0x7C },
    CustomColor { r: 0xFF, g: 0xBF, b: 0x00 },
    CustomColor { r: 0x00, g: 0xA8, b: 0x6B },
    CustomColor { r: 0xFF, g: 0x7F, b: 0x50 },
    CustomColor { r: 0xFF, g: 0xFA, b: 0xCD },
    CustomColor { r: 0x22, g: 0x8B, b: 0x22 },
    CustomColor { r: 0x19, g: 0x19, b: 0x70 },
    CustomColor { r: 0x80, g: 0x00, b: 0x20 },
    CustomColor { r: 0xCC, g: 0xCC, b: 0xFF },
    CustomColor { r: 0xE0, g: 0xB0, b: 0xFF },
    CustomColor { r: 0x78, g: 0xC7, b: 0xC7 },
    CustomColor { r: 0x61, g: 0x40, b: 0x51 },
    CustomColor { r: 0xFF, g: 0xDB, b: 0x58 },
    CustomColor { r: 0xFF, g: 0x7F, b: 0xAA },
    CustomColor { r: 0x4B, g: 0x00, b: 0x82 },
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

    TOKEN_LEFT_PARENTHESIS,  // (
    TOKEN_RIGHT_PARENTHESIS, // )

    TOKEN_PLUS_PLUS,     // ++
    TOKEN_LEFT_BRACKET,  // [
    TOKEN_RIGHT_BRACKET, // ]

    TOKEN_EQUAL_MORE,       // =>
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

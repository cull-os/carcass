use std::fmt;

use rowan::ast::AstNode as _;

use crate::{
    token,
    Kind,
    Kind::*,
    Language,
    RowanElement,
    RowanNode,
    RowanToken,
    Token,
};

pub trait Node: rowan::ast::AstNode<Language = Language> {
    fn nth<N: Node>(&self, n: usize) -> Option<N> {
        self.syntax().children().filter_map(N::cast).nth(n)
    }

    fn children<N: Node>(&self) -> rowan::ast::AstChildren<N> {
        rowan::ast::support::children(self.syntax())
    }

    fn token<T: Token>(&self) -> Option<T> {
        self.children_tokens().next()
    }

    fn token_untyped(&self, kind: Kind) -> Option<RowanToken> {
        self.children_tokens_untyped().find(|it| it.kind() == kind)
    }

    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T> {
        self.syntax()
            .children_with_tokens()
            .filter_map(RowanElement::into_token)
            .filter_map(T::cast)
    }

    fn children_tokens_untyped(&self) -> impl Iterator<Item = RowanToken> {
        self.syntax()
            .children_with_tokens()
            .filter_map(RowanElement::into_token)
    }
}

impl<T: rowan::ast::AstNode<Language = Language>> Node for T {}

macro_rules! node {
    (
        #[from($kind:ident)]
        $(#[$meta:meta])*
        $visibility:vis struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $visibility struct $name(pub RowanNode);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: Kind) -> bool {
                kind == $kind
            }

            fn cast(from: RowanNode) -> Option<Self> {
                Self::can_cast(from.kind()).then_some(Self(from))
            }

            fn syntax(&self) -> &RowanNode {
                &self.0
            }
        }

        impl $name {
            pub const KIND: Kind = $kind;
        }
    };
    (
        #[from($($variant:ident),* $(,)?)]
        $visibility:vis enum $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $visibility enum $name {
             $($variant($variant),)*
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: Kind) -> bool {
                matches!(kind, $($variant::KIND)|*)
            }

            fn cast(from: RowanNode) -> Option<Self> {
                Some(match from.kind() {
                    $($variant::KIND => Self::$variant($variant(from)),)*
                    _ => return None,
                })
            }

            fn syntax(&self) -> &RowanNode {
                match self {
                    $(Self::$variant(this) => &this.0,)*
                }
            }
        }

        $(
            impl From<$variant> for $name {
                fn from(from: $variant) -> Self {
                    Self::$variant(from)
                }
            }

            impl TryFrom<$name> for $variant {
                type Error = ();

                fn try_from(from: $name) -> Result<Self, Self::Error> {
                    match from {
                        $name::$variant(it) => Ok(it),
                        _ => Err(()),
                    }
                }
            }
        )*
    };
}

macro_rules! get_token {
    ($(#[$meta:meta])* $visibility:vis fn $name:ident() -> ? $token:ident) => {
        $(#[$meta])*
        $visibility fn $name(&self) -> Option<RowanToken> {
            self.token_untyped($token)
        }
    };

    ($(#[$meta:meta])* $visibility:vis fn $name:ident() -> $token:ident) => {
        $(#[$meta])*
        $visibility fn $name(&self) -> RowanToken {
            self.token_untyped($token).unwrap()
        }
    };
}

macro_rules! get_node {
    ($(#[$meta:meta])* $visibility:vis fn $name:ident() -> $n:literal @ ? $type:ty) => {
        $(#[$meta])*
        $visibility fn $name(&self) -> Option<$type> {
            self.nth($n)
        }
    };

    ($(#[$meta:meta])* $visibility:vis fn $name:ident() -> $n:literal @ $type:ty) => {
        $(#[$meta])*
        $visibility fn $name(&self) -> $type {
            self.nth($n).unwrap()
        }
    };

    ($(#[$meta:meta])* $visibility:vis fn $name:ident() -> [$type:ty]) => {
        $(#[$meta])*
        $visibility fn $name(&self) -> rowan::ast::AstChildren<$type> {
            self.children()
        }
    };
}

node! { #[from(NODE_ERROR)] pub struct Error; }

// EXPRESSION

node! {
    #[from(
        Parenthesis,
        List,
        AttributeSet,
        Use,
        Lambda,
        Application,
        PrefixOperation,
        InfixOperation,
        Path,
        Identifier,
        String,
        Island,
        Number,
        IfElse,
    )]
    pub enum Expression;
}

// PARENTHESIS

node! { #[from(NODE_PARENTHESIS)] pub struct Parenthesis; }

impl Parenthesis {
    get_token! { pub fn left_parenthesis() -> TOKEN_LEFT_PARENTHESIS }

    get_node! { pub fn expression() -> 0 @ Expression }

    get_token! { pub fn right_parenthesis() -> TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] pub struct List; }

impl List {
    get_token! { pub fn left_bracket() -> TOKEN_LEFT_BRACKET }

    get_node! { pub fn items() -> [Expression] }

    get_token! { pub fn right_bracket() -> TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE SET

node! { #[from(NODE_ATTRIBUTE_SET)] pub struct AttributeSet; }

impl AttributeSet {
    get_token! { pub fn left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { pub fn inherits() -> [AttributeInherit] }

    get_node! { pub fn entries() -> [Attribute] }

    get_token! { pub fn right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_ATTRIBUTE_INHERIT)] pub struct AttributeInherit; }

impl AttributeInherit {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE)] pub struct Attribute; }

impl Attribute {
    get_node! { pub fn path() -> 0 @ AttributePath }

    get_node! { pub fn value() -> 0 @ Expression }

    get_token! { pub fn semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE_PATH)] pub struct AttributePath; }

impl AttributePath {
    get_node! { pub fn identifiers() -> [Identifier] }
}

// BIND

node! { #[from(NODE_BIND)] pub struct Bind; }

impl Bind {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn at() -> TOKEN_AT }
}

// USE

node! { #[from(NODE_USE)] pub struct Use; }

impl Use {
    get_node! { pub fn bind() -> 0 @ ? Bind }

    get_node! { pub fn left_expression() -> 0 @ Expression }

    get_token! { pub fn right_long_arrow() -> TOKEN_EQUAL_EQUAL_MORE }

    get_node! { pub fn right_expression() -> 1 @ Expression }
}

// LAMBDA

node! { #[from(NODE_LAMBDA)] pub struct Lambda; }

impl Lambda {
    get_node! { pub fn parameter() -> 0 @ LambdaParameter }

    get_token! { pub fn colon() -> TOKEN_COLON }

    get_node! { pub fn expression() -> 0 @ Expression }
}

node! {
    #[from(
        LambdaParameterIdentifier,
        LambdaParameterPattern,
    )]
    pub enum LambdaParameter;
}

node! { #[from(NODE_LAMBDA_PARAMETER_IDENTIFIER)] pub struct LambdaParameterIdentifier; }

impl LambdaParameterIdentifier {
    get_node! { pub fn identifier() -> 0 @ Identifier }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN)] pub struct LambdaParameterPattern; }

#[rustfmt::skip]
impl LambdaParameterPattern {
    get_node! { pub fn bind() -> 0 @ ? Bind }

    get_token! { pub fn left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { pub fn entries() -> [LambdaParameterPatternEntry] }

    get_token! { pub fn right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN_ENTRY)] pub struct LambdaParameterPatternEntry; }

impl LambdaParameterPatternEntry {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn questionmark() -> ? TOKEN_QUESTIONMARK }

    get_node! { pub fn default() -> 1 @ ? Expression }
}

// APPLICATION

node! { #[from(NODE_APPLICATION)] pub struct Application; }

impl Application {
    get_node! { pub fn left_expression() -> 0 @ Expression }

    get_node! { pub fn right_expression() -> 1 @ Expression }
}

// PREFIX OPERATION

node! { #[from(NODE_PREFIX_OPERATION)] pub struct PrefixOperation; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
    Swwallation, // Get it?
    Negation,

    Not,
}

impl TryFrom<Kind> for PrefixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_PLUS => Ok(Self::Swwallation),
            TOKEN_MINUS => Ok(Self::Negation),

            TOKEN_LITERAL_NOT => Ok(Self::Not),

            _ => Err(()),
        }
    }
}

#[rustfmt::skip]
impl PrefixOperation {
    pub fn operator(&self) -> PrefixOperator {
        self.children_tokens_untyped()
            .find_map(|token| PrefixOperator::try_from(token.kind()).ok()).unwrap()
    }

    get_node! { pub fn expression() -> 0 @ Expression }
}

// INFIX OPERATION

node! { #[from(NODE_INFIX_OPERATION)] pub struct InfixOperation; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InfixOperator {
    Apply, // <|
    Pipe,  // |>

    Concat, // ++

    Override, // <==
    Update,   // //

    Equal,       // ==
    NotEqual,    // !=
    LessOrEqual, // <=
    Less,        // <
    MoreOrEqual, // >=
    More,        // >
    Implication, // ->

    Addition,       // +
    Negation,       // -
    Multiplication, // *
    Power,          // **
    Division,       // /

    And, // and
    Or,  // or
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_LESS_PIPE => Ok(Self::Apply),
            TOKEN_PIPE_GREATER => Ok(Self::Pipe),

            TOKEN_PLUS_PLUS => Ok(Self::Concat),

            TOKEN_LESS_EQUAL_EQUAL => Ok(Self::Override),
            TOKEN_SLASH_SLASH => Ok(Self::Update),

            TOKEN_EQUAL_EQUAL => Ok(Self::Equal),
            TOKEN_EXCLAMATION_EQUAL => Ok(Self::NotEqual),
            TOKEN_LESS_EQUAL => Ok(Self::LessOrEqual),
            TOKEN_LESS => Ok(Self::Less),
            TOKEN_MORE_EQUAL => Ok(Self::MoreOrEqual),
            TOKEN_MORE => Ok(Self::More),
            TOKEN_MINUS_GREATER => Ok(Self::Implication),

            TOKEN_PLUS => Ok(Self::Addition),
            TOKEN_MINUS => Ok(Self::Negation),
            TOKEN_ASTERISK => Ok(Self::Multiplication),
            TOKEN_ASTERISK_ASTERISK => Ok(Self::Power),
            TOKEN_SLASH => Ok(Self::Division),

            TOKEN_LITERAL_AND => Ok(Self::And),
            TOKEN_LITERAL_OR => Ok(Self::Or),

            _ => Err(()),
        }
    }
}

#[rustfmt::skip]
impl InfixOperation {
    get_node! { pub fn left_expression() -> 0 @ Expression }

    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok()).unwrap()
    }

    get_node! { pub fn right_expression() -> 1 @ Expression }
}

// INTERPOLATION

node! { #[from(NODE_INTERPOLATION)] pub struct Interpolation; }

impl Interpolation {
    get_token! { pub fn interpolation_start() -> TOKEN_INTERPOLATION_START }

    get_node! { pub fn expression() -> 0 @ Expression }

    get_token! { pub fn interpolation_end() -> TOKEN_INTERPOLATION_END }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InterpolationPart<T> {
    Content(T),
    Interpolation(Interpolation),
}

macro_rules! parted {
    ($variant:ident + $type:ty) => {
        pub fn parts(&self) -> impl Iterator<Item = InterpolationPart<$type>> {
            self.syntax().children_with_tokens().map(|child| {
                match child {
                    rowan::NodeOrToken::Token(token) => {
                        debug_assert_eq!(token.kind(), $variant);

                        InterpolationPart::Content(<$type>::cast(token).unwrap())
                    },

                    rowan::NodeOrToken::Node(node) => {
                        debug_assert_eq!(node.kind(), TOKEN_INTERPOLATION_START);

                        InterpolationPart::Interpolation(Interpolation::cast(node.clone()).unwrap())
                    },
                }
            })
        }
    };
}

// PATH, IDENTIFIER, STRING, ISLAND

node! { #[from(NODE_PATH)] pub struct Path; }

impl Path {
    parted! { TOKEN_PATH + token::PathContent }
}

node! { #[from(NODE_IDENTIFIER)] pub struct Identifier; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdentifierValue {
    Simple(token::IdentifierSimple),
    Complex(IdentifierComplex),
}

node! { #[from(NODE_IDENTIFIER)] pub struct IdentifierComplex; }

impl IdentifierComplex {
    parted! { TOKEN_CONTENT + token::Content }
}

impl Identifier {
    pub fn value(&self) -> IdentifierValue {
        if let Some(token) = self.token() {
            return IdentifierValue::Simple(token);
        }

        if self.token_untyped(TOKEN_IDENTIFIER_START).is_some() {
            return IdentifierValue::Complex(IdentifierComplex(self.0.clone_subtree()));
        }

        unreachable!()
    }
}

node! { #[from(NODE_STRING)] pub struct String; }

impl String {
    parted! { TOKEN_CONTENT + token::Content }
}

node! { #[from(NODE_ISLAND)] pub struct Island; }

impl Island {
    parted! { TOKEN_CONTENT + token::Content }
}

// NUMBER

node! { #[from(NODE_NUMBER)] pub struct Number; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumberValue {
    Integer(token::Integer),
    Float(token::Float),
}

impl Number {
    pub fn value(&self) -> NumberValue {
        if let Some(token) = self.token() {
            return NumberValue::Integer(token);
        }

        if let Some(token) = self.token() {
            return NumberValue::Float(token);
        }

        unreachable!()
    }
}

node! { #[from(NODE_IF_ELSE)] pub struct IfElse; }

impl IfElse {
    get_token! { pub fn if_() -> TOKEN_LITERAL_IF }

    get_node! { pub fn condition() -> 0 @ Expression }

    get_token! { pub fn then() -> TOKEN_LITERAL_THEN }

    get_node! { pub fn true_expression() -> 1 @ Expression }

    get_token! { pub fn else_() -> ? TOKEN_LITERAL_ELSE }

    get_node! { pub fn fale_expression() -> 2 @ ? Expression }
}

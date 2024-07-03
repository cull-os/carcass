use std::fmt;

use rowan::ast::AstNode;

use super::{
    token,
    Token,
};
use crate::{
    syntax::{
        self,
        Kind::*,
    },
    Language,
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

    fn token_untyped(&self, kind: syntax::Kind) -> Option<syntax::Token> {
        self.children_tokens_untyped().find(|it| it.kind() == kind)
    }

    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T> {
        self.syntax()
            .children_with_tokens()
            .filter_map(syntax::Element::into_token)
            .filter_map(T::cast)
    }

    fn children_tokens_untyped(&self) -> impl Iterator<Item = syntax::Token> {
        self.syntax()
            .children_with_tokens()
            .filter_map(syntax::Element::into_token)
    }
}

impl<T: rowan::ast::AstNode<Language = Language>> Node for T {}

macro_rules! node {
    (
        #[from($kind:ident)]
        $(#[$meta:meta])*
        struct $name:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub syntax::Node);

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: syntax::Kind) -> bool {
                kind == $kind
            }

            fn cast(from: syntax::Node) -> Option<Self> {
                Self::can_cast(from.kind()).then_some(Self(from))
            }

            fn syntax(&self) -> &syntax::Node {
                &self.0
            }
        }

        impl $name {
            pub const KIND: syntax::Kind = $kind;
        }
    };
    (
        #[from($($variant:ident),* $(,)?)]
        $(#[$meta:meta])*
        enum $name:ident;
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
             $($variant($variant),)*
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(<Self as rowan::ast::AstNode>::syntax(self), formatter)
            }
        }

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: syntax::Kind) -> bool {
                matches!(kind, $($variant::KIND)|*)
            }

            fn cast(from: syntax::Node) -> Option<Self> {
                Some(match from.kind() {
                    $($variant::KIND => Self::$variant($variant(from)),)*
                    _ => return None,
                })
            }

            fn syntax(&self) -> &syntax::Node {
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
    ($name:ident() -> $token:ident) => {
        pub fn $name(&self) -> syntax::Token {
            self.token_untyped($token).unwrap()
        }
    };

    ($name:ident() -> ? $token:ident) => {
        pub fn $name(&self) -> Option<syntax::Token> {
            self.token_untyped($token)
        }
    };
}

macro_rules! get_node {
    ($name:ident() -> $n:literal @ $type:ty) => {
        pub fn $name(&self) -> $type {
            self.nth($n).unwrap()
        }
    };

    ($name:ident() -> $n:literal @ ? $type:ty) => {
        pub fn $name(&self) -> Option<$type> {
            self.nth($n)
        }
    };

    ($name:ident() -> [$type:ty]) => {
        pub fn $name(&self) -> rowan::ast::AstChildren<$type> {
            self.children()
        }
    };
}

node! { #[from(NODE_ERROR)] struct Error; }

// EXPRESSION

node! {
    #[from(
        Parentehsis,
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
    )] enum Expression;
}

// PARENTHESIS

node! { #[from(NODE_PARENTHESIS)] struct Parenthesis; }

impl Parenthesis {
    get_token! { left_parenthesis() -> TOKEN_LEFT_PARENTHESIS }

    get_node! { expression() -> 0 @ Expression }

    get_token! { right_parenthesis() -> TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

impl List {
    get_token! { left_bracket() -> TOKEN_LEFT_BRACKET }

    get_node! { items() -> [Expression] }

    get_token! { right_bracket() -> TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE SET

node! { #[from(NODE_ATTRIBUTE_SET)] struct AttributeSet; }

impl AttributeSet {
    get_token! { left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { inherits() -> [AttributeInherit] }

    get_node! { entries() -> [Attribute] }

    get_token! { right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_ATTRIBUTE_INHERIT)] struct AttributeInherit; }

impl AttributeInherit {
    get_node! { identifier() -> 0 @ Identifier }

    get_token! { semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE)] struct Attribute; }

impl Attribute {
    get_node! { path() -> 0 @ AttributePath }

    get_node! { value() -> 0 @ AttributeValue }

    get_token! { semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE_PATH)] struct AttributePath; }

impl AttributePath {
    get_node! { identifiers() -> [Identifier] }
}

node! { #[from(NODE_ATTRIBUTE_VALUE)] struct AttributeValue; }

impl AttributeValue {
    get_node! { value() -> 0 @ Expression }
}

// BIND

node! { #[from(NODE_BIND)] struct Bind; }

impl Bind {
    get_node! { identifier() -> 0 @ Identifier }

    get_token! { at() -> TOKEN_AT }
}

// USE

node! { #[from(NODE_USE)] struct Use; }

impl Use {
    get_node! { bind() -> 0 @ ? Bind }

    get_node! { left_expression() -> 0 @ Expression }

    get_token! { right_long_arrow() -> TOKEN_EQUAL_EQUAL_MORE }

    get_node! { right_expression() -> 1 @ Expression }
}

// LAMBDA

node! { #[from(NODE_LAMBDA)] struct Lambda; }

impl Lambda {
    get_node! { parameter() -> 0 @ LambdaParameter }

    get_token! { colon() -> TOKEN_COLON }

    get_node! { expression() -> 0 @ Expression }
}

node! {
    #[from(
        Identifier,
        LambdaParameterPattern,
    )] enum LambdaParameter;
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN)] struct LambdaParameterPattern; }

impl LambdaParameterPattern {
    get_node! { bind() -> 0 @ ? Bind }

    get_token! { left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { entries() -> [LambdaParameterPatternEntry] }

    get_token! { right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN_ENTRY)] struct LambdaParameterPatternEntry; }

impl LambdaParameterPatternEntry {
    get_node! { identifier() -> 0 @ Identifier }

    get_token! { questionmark() -> ? TOKEN_QUESTIONMARK }

    get_node! { default() -> 1 @ ? Expression }
}

// APPLICATION

node! { #[from(NODE_APPLICATION)] struct Applicaton; }

impl Applicaton {
    get_node! { left_expression() -> 0 @ Expression }

    get_node! { right_expression() -> 1 @ Expression }
}

// PREFIX OPERATION

node! { #[from(NODE_PREFIX_OPERATION)] struct PrefixOperation; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
    Swwallation, // Get it?
    Negation,

    Not,
}

impl TryFrom<syntax::Kind> for PrefixOperator {
    type Error = ();

    fn try_from(from: syntax::Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_PLUS => Ok(Self::Swwallation),
            TOKEN_MINUS => Ok(Self::Negation),

            TOKEN_NOT => Ok(Self::Not),

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

    get_node! { expression() -> 0 @ Expression }
}

// INFIX OPERATION

node! { #[from(NODE_INFIX_OPERATION)] struct InfixOperation; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum InfixOperator {
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

impl TryFrom<syntax::Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: syntax::Kind) -> Result<Self, Self::Error> {
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
    get_node! { left_expression() -> 0 @ Expression }

    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok()).unwrap()
    }

    get_node! { right_expression() -> 1 @ Expression }
}

// INTERPOLATION

node! { #[from(NODE_INTERPOLATION)] struct Interpolation; }

impl Interpolation {
    get_token! { interpolation_start() -> TOKEN_INTERPOLATION_START }

    get_node! { expression() -> 0 @ Expression }

    get_token! { interpolation_end() -> TOKEN_INTERPOLATION_END }
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

node! { #[from(TOKEN_PATH)] struct Path; }

impl Path {
    parted! { TOKEN_PATH + token::PathContent }
}

node! { #[from(NODE_IDENTIFIER)] struct Identifier; }

impl Identifier {
    parted! { TOKEN_IDENTIFIER_CONTENT + token::IdentifierContent }
}

node! { #[from(NODE_STRING)] struct String; }

impl String {
    parted! { TOKEN_STRING_CONTENT + token::StringContent }
}

node! { #[from(NODE_ISLAND)] struct Island; }

impl Island {
    parted! { TOKEN_ISLAND_CONTENT + token::IslandContent }
}

// NUMBER

node! { #[from(NODE_NUMBER)] struct Number; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum NumberType {
    Integer,
    Float,
}

impl Number {
    pub fn type_(&self) -> NumberType {
        match self.syntax().kind() {
            TOKEN_INTEGER => NumberType::Integer,
            TOKEN_FLOAT => NumberType::Float,
            _ => unreachable!(),
        }
    }
}

node! { #[from(NODE_IF_ELSE)] struct IfElse; }

impl IfElse {
    get_token! { if_() -> TOKEN_LITERAL_IF }

    get_node! { condition() -> 0 @ Expression }

    get_token! { then() -> TOKEN_LITERAL_THEN }

    get_node! { true_expression() -> 1 @ Expression }

    get_token! { else_() -> ? TOKEN_LITERAL_ELSE }

    get_node! { fale_expression() -> 2 @ ? Expression }
}

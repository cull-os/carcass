//! [`Node`] definitions for the Cab language.
use std::ops::{
    self,
    Not as _,
};

use rowan::ast::AstNode as _;

use crate::{
    token::{
        self,
        Token,
    },
    Kind::{
        self,
        *,
    },
    Language,
    RowanElement,
    RowanNode,
    RowanToken,
};

/// A macro that allows you to match on a [`rowan::SyntaxNode`] efficiently.
///
/// The branches must all implement [`Node`] for this macro to work properly.
///
/// # Example
///
/// ```ignore
/// match_node! { rowan_node =>
///     IfElse:(if_else) => { unimplemented!() },
///     Identifier:(identifier) => { unimplemented!() },
///     _ => unimplemented!(),
/// }
/// ```
#[macro_export]
macro_rules! match_node {
    ($raw:expr =>
        $($typed:ty:($name:ident) => $result:expr,)*
        _ => $catch:expr $(,)?
    ) => {
        $(if $typed::can_cast($raw) {
            let $name = $typed::cast($raw.clone()).unwrap();
            $result
        } else )*{
            $catch
        }
    };
}

pub trait Node: rowan::ast::AstNode<Language = Language> + ops::Deref<Target = RowanNode> {
    /// Returns the Nth immediate children node that can be cast to the given
    /// typed node.
    fn nth<N: Node>(&self, n: usize) -> Option<N> {
        self.children::<N>().nth(n)
    }

    /// Returns all immediate children nodes that can be cast to the given typed
    /// node.
    fn children<N: Node>(&self) -> rowan::ast::AstChildren<N> {
        rowan::ast::support::children(self.syntax())
    }

    /// Returns the first immediate children token that can be cast to the given
    /// typed token.
    fn token<T: Token>(&self) -> Option<T> {
        self.children_tokens().next()
    }

    /// Returns the first immediate children token that is the given kind.
    fn token_untyped(&self, kind: Kind) -> Option<RowanToken> {
        self.children_tokens_untyped().find(|it| it.kind() == kind)
    }

    /// Returns all immediate children tokens that can be cast to the given
    /// typed token.
    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T> {
        self.children_with_tokens()
            .filter_map(RowanElement::into_token)
            .filter_map(T::cast)
    }

    /// Returns all immediate children tokens.
    fn children_tokens_untyped(&self) -> impl Iterator<Item = RowanToken> {
        self.children_with_tokens()
            .filter_map(RowanElement::into_token)
    }
}

impl<T: rowan::ast::AstNode<Language = Language> + ops::Deref<Target = RowanNode>> Node for T {}

macro_rules! node {
    (
        #[from($kind:ident)]
        struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) RowanNode);

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

        impl ops::Deref for $name {
            type Target = RowanNode;

            fn deref(&self) -> &Self::Target {
                self.syntax()
            }
        }

        impl $name {
            /// The syntax kind this node can be cast from.
            pub const KIND: Kind = $kind;
        }
    };

    (
        #[from($($variant:ident),* $(,)?)]
        enum $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant($variant),)*
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
                    $(Self::$variant(this) => &this.syntax(),)*
                }
            }
        }

        impl ops::Deref for $name {
            type Target = RowanNode;

            fn deref(&self) -> &Self::Target {
                self.syntax()
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
    ($name:ident -> ? $token:ident) => {
        pub fn $name(&self) -> Option<RowanToken> {
            self.token_untyped($token)
        }
    };

    ($name:ident -> $token:ident) => {
        pub fn $name(&self) -> RowanToken {
            self.token_untyped($token).unwrap()
        }
    };
}

macro_rules! get_node {
    ($name:ident -> $n:literal @ ? $type:ty) => {
        pub fn $name(&self) -> Option<$type> {
            self.nth($n)
        }
    };

    ($name:ident -> $n:literal @ $type:ty) => {
        pub fn $name(&self) -> $type {
            self.nth($n).unwrap()
        }
    };

    ($name:ident -> [$type:ty]) => {
        pub fn $name(&self) -> rowan::ast::AstChildren<$type> {
            self.children()
        }
    };
}

node! { #[from(NODE_ROOT)] struct Root; }

impl Root {
    get_node! { expression -> 0 @ Expression }
}

// EXPRESSION

node! {
    #[from(
        Error,
        Parenthesis,
        List,
        AttributeSet,
        AttributeSelect,
        AttributeCheck,
        Bind,
        Lambda,
        Application,
        PrefixOperation,
        InfixOperation,
        Path,
        Identifier,
        SString,
        Island,
        Number,
        IfElse,
    )]
    enum Expression;
}

// ERROR

node! { #[from(NODE_ERROR)] struct Error; }

// PARENTHESIS

node! { #[from(NODE_PARENTHESIS)] struct Parenthesis; }

impl Parenthesis {
    get_token! { left_parenthesis -> TOKEN_LEFT_PARENTHESIS }

    get_node! { expression -> 0 @ Expression }

    get_token! { right_parenthesis -> TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

impl List {
    get_token! { left_bracket -> TOKEN_LEFT_BRACKET }

    get_node! { items -> [Expression] }

    get_token! { right_bracket -> TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE SET

node! { #[from(NODE_ATTRIBUTE_SET)] struct AttributeSet; }

impl AttributeSet {
    get_token! { left_curlybrace -> TOKEN_LEFT_CURLYBRACE }

    get_node! { inherits -> [AttributeInherit] }

    get_node! { attributes -> [Attribute] }

    get_token! { right_curlybrace -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_ATTRIBUTE_INHERIT)] struct AttributeInherit; }

impl AttributeInherit {
    get_node! { identifier -> 0 @ Identifier }

    get_token! { semicolon -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE)] struct Attribute; }

impl Attribute {
    get_node! { path -> 0 @ AttributePath }

    get_node! { value -> 0 @ Expression }

    get_token! { semicolon -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE_PATH)] struct AttributePath; }

impl AttributePath {
    get_node! { identifiers -> [Identifier] }
}

// ATTRIBUTE SELECT

node! { #[from(NODE_ATTRIBUTE_SELECT)] struct AttributeSelect; }

#[rustfmt::skip]
impl AttributeSelect {
    get_node! { expression -> 0 @ Expression }

    pub fn identifier(&self) -> Identifier {
        let expression: Expression = self.nth(1).unwrap();
        Identifier::cast(expression.syntax().clone()).unwrap()
    }

    get_node! { default -> 2 @ ? Expression }
}

// ATTRIBUTE CHECK

node! { #[from(NODE_ATTRIBUTE_CHECK)] struct AttributeCheck; }

impl AttributeCheck {
    get_node! { expression -> 0 @ Expression }

    pub fn attributes(&self) -> impl Iterator<Item = Identifier> {
        let expressions: rowan::ast::AstChildren<Expression> = self.children();

        expressions
            .skip(1)
            .map(|expression| Identifier::cast(expression.syntax().clone()).unwrap())
    }
}

// BIND

node! { #[from(NODE_BIND)] struct Bind; }

impl Bind {
    get_node! { identifier -> 0 @ Identifier }

    get_token! { at -> TOKEN_AT }

    get_node! { expression -> 1 @ Expression }
}

// LAMBDA

node! { #[from(NODE_LAMBDA)] struct Lambda; }

impl Lambda {
    get_node! { parameter -> 0 @ LambdaParameter }

    get_token! { colon -> TOKEN_COLON }

    get_node! { expression -> 0 @ Expression }
}

node! {
    #[from(
        LambdaParameterIdentifier,
        LambdaParameterPattern,
    )]
    enum LambdaParameter;
}

node! { #[from(NODE_LAMBDA_PARAMETER_IDENTIFIER)] struct LambdaParameterIdentifier; }

impl LambdaParameterIdentifier {
    get_node! { identifier -> 0 @ Identifier }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN)] struct LambdaParameterPattern; }

#[rustfmt::skip]
impl LambdaParameterPattern {
    get_token! { left_curlybrace -> TOKEN_LEFT_CURLYBRACE }

    get_node! { attributes -> [LambdaParameterPatternAttribute] }

    get_token! { right_curlybrace -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN_ATTRIBUTE)] struct LambdaParameterPatternAttribute; }

impl LambdaParameterPatternAttribute {
    get_node! { identifier -> 0 @ Identifier }

    get_token! { questionmark -> ? TOKEN_QUESTIONMARK }

    get_node! { default -> 1 @ ? Expression }
}

// APPLICATION

node! { #[from(NODE_APPLICATION)] struct Application; }

impl Application {
    get_node! { left_expression -> 0 @ Expression }

    get_node! { right_expression -> 1 @ Expression }
}

// PREFIX OPERATION

node! { #[from(NODE_PREFIX_OPERATION)] struct PrefixOperation; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    get_node! { expression -> 0 @ Expression }
}

// INFIX OPERATION

node! { #[from(NODE_INFIX_OPERATION)] struct InfixOperation; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfixOperator {
    Apply,
    Pipe,

    Concat,

    Use,
    Override,
    Update,

    Equal,
    NotEqual,
    LessOrEqual,
    Less,
    MoreOrEqual,
    More,
    Implication,

    Addition,
    Negation,
    Multiplication,
    Power,
    Division,

    And,
    Or,
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_DOLLAR => Ok(Self::Apply),
            TOKEN_PIPE_MORE => Ok(Self::Pipe),

            TOKEN_PLUS_PLUS => Ok(Self::Concat),

            TOKEN_EQUAL_EQUAL_MORE => Ok(Self::Use),
            TOKEN_LESS_EQUAL_EQUAL => Ok(Self::Override),
            TOKEN_SLASH_SLASH => Ok(Self::Update),

            TOKEN_EQUAL_EQUAL => Ok(Self::Equal),
            TOKEN_EXCLAMATION_EQUAL => Ok(Self::NotEqual),
            TOKEN_LESS_EQUAL => Ok(Self::LessOrEqual),
            TOKEN_LESS => Ok(Self::Less),
            TOKEN_MORE_EQUAL => Ok(Self::MoreOrEqual),
            TOKEN_MORE => Ok(Self::More),
            TOKEN_MINUS_MORE => Ok(Self::Implication),

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

impl InfixOperator {
    pub fn binding_power(self) -> (u16, u16) {
        match self {
            Self::Concat => (140, 145),
            Self::Multiplication | Self::Power | Self::Division => (130, 135),
            Self::Addition | Self::Negation => (120, 125),
            Self::Override => (110, 115),
            Self::Use => (100, 105),
            Self::Update => (90, 95),
            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More => (80, 85),
            Self::Equal | Self::NotEqual => (70, 75),
            Self::And => (60, 65),
            Self::Or => (50, 55),
            Self::Implication => (30, 35),
            Self::Apply => (20, 25),
            Self::Pipe => (10, 15),
        }
    }
}

#[rustfmt::skip]
impl InfixOperation {
    get_node! { left_expression -> 0 @ Expression }

    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok()).unwrap()
    }

    get_node! { right_expression -> 1 @ Expression }
}

// INTERPOLATION

node! { #[from(NODE_INTERPOLATION)] struct Interpolation; }

impl Interpolation {
    get_token! { interpolation_start -> TOKEN_INTERPOLATION_START }

    get_node! { expression -> 0 @ Expression }

    get_token! { interpolation_end -> TOKEN_INTERPOLATION_END }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterpolationPart<T> {
    Delimiter(RowanToken),
    Content(T),
    Interpolation(Interpolation),
}

macro_rules! parted {
    (
        impl
        $name:ident { let content_kind = $content_kind:expr; type ContentToken = $content_token:ty; }
    ) => {
        impl $name {
            pub fn parts(&self) -> impl Iterator<Item = InterpolationPart<$content_token>> {
                self.children_with_tokens().filter_map(|child| {
                    match child {
                        rowan::NodeOrToken::Token(token) => {
                            token.kind().is_trivia().not().then(|| {
                                if token.kind() == $content_kind {
                                    InterpolationPart::Content(
                                        <$content_token>::cast(token).unwrap(),
                                    )
                                } else {
                                    InterpolationPart::Delimiter(token)
                                }
                            })
                        },

                        rowan::NodeOrToken::Node(node) => {
                            debug_assert_eq!(node.kind(), NODE_INTERPOLATION);

                            Some(InterpolationPart::Interpolation(
                                Interpolation::cast(node.clone()).unwrap(),
                            ))
                        },
                    }
                })
            }
        }
    };
}

// PATH, IDENTIFIER, STRING, ISLAND

node! { #[from(NODE_PATH)] struct Path; }

parted! {
    impl Path {
        let content_kind = TOKEN_PATH;
        type ContentToken = token::Path;
    }
}

node! { #[from(NODE_IDENTIFIER)] struct Identifier; }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdentifierValue {
    Simple(token::Identifier),
    Complex(IdentifierComplex),
}

node! { #[from(NODE_IDENTIFIER)] struct IdentifierComplex; }

parted! {
    impl IdentifierComplex {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

impl Identifier {
    pub fn value(&self) -> IdentifierValue {
        if let Some(token) = self.token() {
            return IdentifierValue::Simple(token);
        }

        if self.token_untyped(TOKEN_IDENTIFIER_START).is_some() {
            return IdentifierValue::Complex(IdentifierComplex(self.syntax().clone()));
        }

        unreachable!()
    }
}

node! { #[from(NODE_STRING)] struct SString; }

parted! {
    impl SString {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

node! { #[from(NODE_ISLAND)] struct Island; }

parted! {
    impl Island {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// NUMBER

node! { #[from(NODE_NUMBER)] struct Number; }

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

node! { #[from(NODE_IF_ELSE)] struct IfElse; }

impl IfElse {
    get_token! { r#if -> TOKEN_LITERAL_IF }

    get_node! { condition -> 0 @ Expression }

    get_token! { then -> TOKEN_LITERAL_THEN }

    get_node! { true_expression -> 1 @ Expression }

    get_token! { r#else -> ? TOKEN_LITERAL_ELSE }

    get_node! { false_expression -> 2 @ ? Expression }
}

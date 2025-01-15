//! [`Node`] definitions for the Cab language.
use std::{
    collections::VecDeque,
    ops::{
        self,
        Deref as _,
        Not as _,
    },
};

use rowan::ast::AstNode as _;
use static_assertions::assert_obj_safe;

use crate::{
    Kind::{
        self,
        *,
    },
    Language,
    RowanElement,
    RowanNode,
    RowanToken,
    token::{
        self,
        Token,
    },
};

#[macro_export]
#[doc(hidden)]
macro_rules! __node_match {
    ($raw:expr =>
        $($typed:ty as $name:ident => $result:expr,)*
        else => $catch:expr $(,)?
    ) => {{
        use ::rowan::ast::AstNode as _;

        $(if <$typed>::can_cast($raw.kind()) {
            let $name = <$typed>::cast($raw.clone()).unwrap();
            $result
        } else )*{
            $catch
        }
    }};
}

/// A macro that allows you to match on a [`rowan::SyntaxNode`] efficiently.
/// The branches must all implement [`Node`] for this macro to work properly.
///
/// # Example
///
/// ```ignore
/// node::r#match! { rowan_node =>
///    IfElse as if_else => { unimplemented!() },
///    Identifier as identifier => { unimplemented!() },
///    else => unimplemented!(),
/// }
/// ```
#[doc(inline)]
pub use crate::__node_match as r#match;

assert_obj_safe!(Node);
pub trait Node: rowan::ast::AstNode<Language = Language> + ops::Deref<Target = RowanNode> {
    /// Returns its inherent kind, returning None if it is a node that can have
    /// multiple values.
    fn inherent_kind() -> Option<Kind>
    where
        Self: Sized,
    {
        None
    }

    /// Returns the Nth immediate child node that can be cast to the given
    /// typed node.
    fn nth<N: Node>(&self, n: usize) -> Option<N>
    where
        Self: Sized,
    {
        self.children::<N>().nth(n)
    }

    /// Returns all immediate children nodes that can be cast to the given typed
    /// node.
    fn children<N: Node>(&self) -> rowan::ast::AstChildren<N>
    where
        Self: Sized,
    {
        rowan::ast::support::children(self)
    }

    /// Returns the first immediate children token that can be cast to the given
    /// typed token.
    fn token<T: Token>(&self) -> Option<T>
    where
        Self: Sized,
    {
        self.children_tokens().next()
    }

    /// Returns the first immediate children token that is the given kind.
    fn token_untyped(&self, kind: Kind) -> Option<RowanToken>
    where
        Self: Sized,
    {
        self.children_tokens_untyped().find(|it| it.kind() == kind)
    }

    /// Returns all immediate children tokens that can be cast to the given
    /// typed token.
    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T>
    where
        Self: Sized,
    {
        self.children_with_tokens()
            .filter_map(RowanElement::into_token)
            .filter_map(T::cast)
    }

    /// Returns all immediate children tokens.
    fn children_tokens_untyped(&self) -> impl Iterator<Item = RowanToken>
    where
        Self: Sized,
    {
        self.children_with_tokens()
            .filter_map(RowanElement::into_token)
    }
}

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
                kind == Self::KIND
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

        impl Node for $name {
            fn inherent_kind() -> Option<Kind> {
                Some(Self::KIND)
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
                match from.kind() {
                    $($variant::KIND => Some(Self::$variant($variant(from))),)*
                    _ => None,
                }
            }

            fn syntax(&self) -> &RowanNode {
                match self {
                    $(Self::$variant(this) => this,)*
                }
            }
        }

        impl ops::Deref for $name {
            type Target = RowanNode;

            fn deref(&self) -> &Self::Target {
                self.syntax()
            }
        }

        impl Node for $name {}

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

// EXPRESSION

node! {
    #[from(
        Error,
        Parenthesis,
        List,
        AttributeList,
        PrefixOperation,
        InfixOperation,
        SuffixOperation,
        Path,
        Identifier,
        SString,
        Island,
        Number,
        IfIs,
        IfElse,
    )] enum Expression;
}

// ERROR

node! { #[from(NODE_ERROR)] struct Error; }

// PARENTHESIS

node! { #[from(NODE_PARENTHESIS)] struct Parenthesis; }

impl Parenthesis {
    get_token! { left_parenthesis -> TOKEN_LEFT_PARENTHESIS }

    get_node! { expression -> 0 @ Expression }

    get_token! { right_parenthesis -> ? TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

#[rustfmt::skip]
impl List {
    get_token! { left_bracket -> TOKEN_LEFT_BRACKET }

    get_node! { expression -> 0 @ ? Expression }

    pub fn items(&self) -> impl Iterator<Item = Expression> {
        gen {
            let mut expressions: VecDeque<_> = self.expression().into_iter().collect();

            while let Some(expression) = expressions.pop_back() {
                match expression {
                    Expression::InfixOperation(operation)
                        if operation.operator() == InfixOperator::Same =>
                    {
                        expressions.push_front(operation.left_expression());
                        expressions.push_front(operation.right_expression().unwrap());
                    },

                    Expression::SuffixOperation(operation)
                        if operation.operator() == SuffixOperator::Same =>
                    {
                        expressions.push_front(operation.expression());
                    },

                    normal => yield normal,
                }
            }
        }
    }

    get_token! { right_bracket -> ? TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE LIST

node! { #[from(NODE_ATTRIBUTE_LIST)] struct AttributeList; }

impl AttributeList {
    get_token! { left_curlybrace -> TOKEN_LEFT_CURLYBRACE }

    get_node! { expression -> 0 @ ? Expression }

    get_token! { right_curlybrace -> ? TOKEN_RIGHT_CURLYBRACE }
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
        Ok(match from {
            TOKEN_PLUS => Self::Swwallation,
            TOKEN_MINUS => Self::Negation,

            TOKEN_EXCLAMATIONMARK => Self::Not,

            _ => return Err(()),
        })
    }
}

impl PrefixOperator {
    pub fn binding_power(self) -> ((), u16) {
        match self {
            Self::Swwallation | Self::Negation => ((), 145),
            Self::Not => ((), 125),
        }
    }
}

#[rustfmt::skip]
impl PrefixOperation {
    pub fn operator_token(&self) -> Option<RowanToken> {
        self.children_tokens_untyped()
            .find(|token| PrefixOperator::try_from(token.kind()).is_ok())
    }

    pub fn operator(&self) -> PrefixOperator {
        self.children_tokens_untyped()
            .find_map(|token| PrefixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    get_node! { expression -> 0 @ Expression }
}

// INFIX OPERATION

node! { #[from(NODE_INFIX_OPERATION)] struct InfixOperation; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfixOperator {
    Same,
    Sequence,

    ImplicitApply,
    Apply,
    Pipe,

    Concat,

    Select,
    Check,
    Update,

    Equal,
    NotEqual,
    LessOrEqual,
    Less,
    MoreOrEqual,
    More,

    And,
    Or,
    Implication,

    Addition,
    Subtraction,
    Multiplication,
    Power,
    Division,

    Lambda,
    Bind,
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        Ok(match from {
            TOKEN_SEMICOLON => Self::Sequence,
            TOKEN_COMMA => Self::Same,

            kind if kind.is_argument() => Self::ImplicitApply,
            TOKEN_LESS_PIPE => Self::Apply,
            TOKEN_PIPE_MORE => Self::Pipe,

            TOKEN_PLUS_PLUS => Self::Concat,

            TOKEN_PERIOD => Self::Select,
            TOKEN_QUESTIONMARK => Self::Check,
            TOKEN_SLASH_SLASH => Self::Update,

            TOKEN_EQUAL_EQUAL => Self::Equal,
            TOKEN_EXCLAMATION_EQUAL => Self::NotEqual,
            TOKEN_LESS_EQUAL => Self::LessOrEqual,
            TOKEN_LESS => Self::Less,
            TOKEN_MORE_EQUAL => Self::MoreOrEqual,
            TOKEN_MORE => Self::More,

            TOKEN_AMPERSAND_AMPERSAND => Self::And,
            TOKEN_PIPE_PIPE => Self::Or,
            TOKEN_MINUS_MORE => Self::Implication,

            TOKEN_PLUS => Self::Addition,
            TOKEN_MINUS => Self::Subtraction,
            TOKEN_ASTERISK => Self::Multiplication,
            TOKEN_CARET => Self::Power,
            TOKEN_SLASH => Self::Division,

            TOKEN_EQUAL_GREATER => Self::Lambda,
            TOKEN_COLON_EQUAL => Self::Bind,

            _ => return Err(()),
        })
    }
}

impl InfixOperator {
    pub fn binding_power(self) -> (u16, u16) {
        match self {
            Self::Select => (180, 185),
            Self::ImplicitApply => (170, 175),

            Self::Concat => (160, 165),

            Self::Multiplication | Self::Division => (150, 155),
            Self::Power => (155, 150),

            // PrefixOperator::Swallation | PrefixOperator::Negation
            Self::Addition | Self::Subtraction => (130, 135),
            // PrefixOperator::Not
            Self::Update => (110, 115),

            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More | Self::Check => {
                (100, 105)
            },
            Self::Equal | Self::NotEqual => (95, 90),

            Self::And => (85, 80),
            Self::Or => (75, 70),
            Self::Implication => (65, 60),

            Self::Lambda => (55, 50),
            Self::Bind => (45, 40),

            Self::Pipe => (30, 35),
            Self::Apply => (35, 30),

            Self::Same => (25, 20),
            Self::Sequence => (15, 10),
        }
    }

    pub fn is_token_owning(self) -> bool {
        self != Self::ImplicitApply
    }
}

#[rustfmt::skip]
impl InfixOperation {
    get_node! { left_expression -> 0 @ Expression }

    pub fn operator_token(&self) -> Option<RowanToken> {
        self.children_tokens_untyped()
            .find(|token| InfixOperator::try_from(token.kind()).is_ok())
    }

    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok())
            .unwrap_or(InfixOperator::ImplicitApply)
    }

    get_node! { right_expression -> 1 @ ? Expression }
}

// SUFFIX OPERATION

node! { #[from(NODE_SUFFIX_OPERATION)] struct SuffixOperation; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuffixOperator {
    Same,
    Sequence,
}

impl TryFrom<Kind> for SuffixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_COMMA => Ok(Self::Same),
            TOKEN_SEMICOLON => Ok(Self::Sequence),

            _ => Err(()),
        }
    }
}

impl SuffixOperation {
    get_node! { expression -> 0 @ Expression }

    pub fn operator_token(&self) -> Option<RowanToken> {
        self.children_tokens_untyped()
            .find(|token| SuffixOperator::try_from(token.kind()).is_ok())
    }

    pub fn operator(&self) -> SuffixOperator {
        self.children_tokens_untyped()
            .find_map(|token| SuffixOperator::try_from(token.kind()).ok())
            .unwrap()
    }
}

// INTERPOLATION

node! { #[from(NODE_INTERPOLATION)] struct Interpolation; }

impl Interpolation {
    get_token! { interpolation_start -> TOKEN_INTERPOLATION_START }

    get_node! { expression -> 0 @ ? Expression }

    get_token! { interpolation_end -> ? TOKEN_INTERPOLATION_END }
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
                            assert_eq!(node.kind(), NODE_INTERPOLATION);

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
            return IdentifierValue::Complex(IdentifierComplex(self.deref().clone()));
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

// IF IS

node! { #[from(NODE_IF_IS)] struct IfIs; }

impl IfIs {
    get_token! { r#if -> TOKEN_LITERAL_IF }

    get_node! { expression -> 0 @ ? Expression }

    get_token! { is -> ? TOKEN_LITERAL_IS }

    get_node! { match_expression -> 1 @ ? Expression }
}

// IF ELSE

node! { #[from(NODE_IF_ELSE)] struct IfElse; }

impl IfElse {
    get_token! { r#if -> TOKEN_LITERAL_IF }

    get_node! { condition -> 0 @ ? Expression }

    get_token! { then -> ? TOKEN_LITERAL_THEN }

    get_node! { true_expression -> 1 @ ? Expression }

    get_token! { r#else -> ? TOKEN_LITERAL_ELSE }

    get_node! { false_expression -> 2 @ ? Expression }
}

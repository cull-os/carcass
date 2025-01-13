//! [`Node`] definitions for the Cab language.
use std::{
    collections::VecDeque,
    ops::{
        self,
        Not as _,
    },
};

use rowan::ast::AstNode as _;
use static_assertions::assert_obj_safe;

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
        rowan::ast::support::children(self.syntax())
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
        AttributeSet,
        Application,
        PrefixOperation,
        InfixOperation,
        SuffixOperation,
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

    get_token! { right_parenthesis -> ? TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

#[rustfmt::skip]
impl List {
    get_token! { left_bracket -> TOKEN_LEFT_BRACKET }

    get_node! { expression -> 0 @ ? Expression }

    pub fn items(&self) -> Vec<Expression> {
        let mut normals = Vec::new();
        let mut sameables: VecDeque<_> = self.expression().into_iter().collect();

        while let Some(expression) = sameables.pop_back() {
            match expression {
                Expression::InfixOperation(operation)
                    if operation.operator() == InfixOperator::Same =>
                {
                    sameables.push_front(operation.left_expression());

                    if let Some(expression) = operation.right_expression() {
                        sameables.push_front(expression);
                    }
                },

                Expression::SuffixOperation(operation)
                    if operation.operator() == SuffixOperator::Same =>
                {
                    sameables.push_front(operation.expression());
                },

                normal => normals.push(normal),
            }
        }

        normals
    }

    get_token! { right_bracket -> ? TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE SET

node! { #[from(NODE_ATTRIBUTE_SET)] struct AttributeSet; }

impl AttributeSet {
    get_token! { left_curlybrace -> TOKEN_LEFT_CURLYBRACE }

    get_node! { expression -> 0 @ ? Expression }

    get_token! { right_curlybrace -> ? TOKEN_RIGHT_CURLYBRACE }
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

impl PrefixOperator {
    pub fn binding_power(self) -> ((), u16) {
        match self {
            Self::Swwallation | Self::Negation => ((), 185),
            Self::Not => ((), 115),
        }
    }
}

#[rustfmt::skip]
impl PrefixOperation {
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
    Implication,

    Addition,
    Subtraction,
    Multiplication,
    Power,
    Division,

    And,
    Or,

    Lambda,
    Bind,
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_SEMICOLON => Ok(Self::Sequence),
            TOKEN_COMMA => Ok(Self::Same),

            TOKEN_LESS_PIPE => Ok(Self::Apply),
            TOKEN_PIPE_MORE => Ok(Self::Pipe),

            TOKEN_PLUS_PLUS => Ok(Self::Concat),

            TOKEN_PERIOD => Ok(Self::Select),
            TOKEN_QUESTIONMARK => Ok(Self::Check),
            TOKEN_SLASH_SLASH => Ok(Self::Update),

            TOKEN_EQUAL_EQUAL => Ok(Self::Equal),
            TOKEN_EXCLAMATION_EQUAL => Ok(Self::NotEqual),
            TOKEN_LESS_EQUAL => Ok(Self::LessOrEqual),
            TOKEN_LESS => Ok(Self::Less),
            TOKEN_MORE_EQUAL => Ok(Self::MoreOrEqual),
            TOKEN_MORE => Ok(Self::More),
            TOKEN_MINUS_MORE => Ok(Self::Implication),

            TOKEN_PLUS => Ok(Self::Addition),
            TOKEN_MINUS => Ok(Self::Subtraction),
            TOKEN_ASTERISK => Ok(Self::Multiplication),
            TOKEN_CARET => Ok(Self::Power),
            TOKEN_SLASH => Ok(Self::Division),

            TOKEN_LITERAL_AND => Ok(Self::And),
            TOKEN_LITERAL_OR => Ok(Self::Or),

            TOKEN_EQUAL_GREATER => Ok(Self::Lambda),
            TOKEN_COLON_EQUAL => Ok(Self::Bind),

            _ => Err(()),
        }
    }
}

impl InfixOperator {
    pub fn binding_power(self) -> (u16, u16) {
        match self {
            Self::Select => (190, 195),
            // PrefixOperator::Swallation | PrefixOperator::Negation
            Self::Concat => (170, 175),
            Self::Multiplication | Self::Power | Self::Division => (160, 165),
            Self::Addition | Self::Subtraction => (150, 155),
            Self::Update => (140, 145),
            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More | Self::Check => {
                (130, 135)
            },
            Self::Equal | Self::NotEqual => (120, 125),
            // PrefixOperator::Not
            Self::And => (100, 105),
            Self::Or => (90, 95),
            Self::Apply => (80, 75),
            Self::Pipe => (65, 70),
            Self::Implication => (55, 50),
            Self::Lambda => (45, 40),
            Self::Bind => (35, 30),
            Self::Same => (25, 20),
            Self::Sequence => (15, 10),
        }
    }
}

#[rustfmt::skip]
impl InfixOperation {
    get_node! { left_expression -> 0 @ Expression }

    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok())
            .unwrap()
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

    get_node! { condition -> 0 @ ? Expression }

    get_token! { then -> ? TOKEN_LITERAL_THEN }

    get_node! { true_expression -> 1 @ ? Expression }

    get_token! { r#else -> ? TOKEN_LITERAL_ELSE }

    get_node! { false_expression -> 2 @ ? Expression }
}

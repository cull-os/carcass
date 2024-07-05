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

#[macro_export]
macro_rules! match_node {
    ($raw:expr =>
        $($typed:ty:($name:ident) => $result:expr,)*
        _ => $catch:expr $(,)?
    ) => {
        $(if $typed::can_cast($raw) {
            let $name = $typed::cast($raw.clone_subtree()).unwrap();
            $result
        } else )*{
            $catch
        }
    };
}

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
        $visibility:vis struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        $visibility struct $name(pub RowanNode);

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
        #[from($kind:ident)]
        $visibility:vis struct $name:ident => |$self:ident, $formatter:ident| $format_expr:expr
    ) => {
        node! {
            #[from($kind)]
            $visibility struct $name;
        }

        impl fmt::Display for $name {
            fn fmt(&$self, $formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                $format_expr
            }
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
                match self {
                    $(Self::$variant(variant) => write!(formatter, "{variant}"),)*
                }
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

node! { #[from(NODE_ERROR)] pub struct Error => |self, formatter| write!(formatter, "ERROR") }

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

node! { #[from(NODE_PARENTHESIS)] pub struct Parenthesis => |self, formatter| write!(formatter, "({self})") }

impl Parenthesis {
    get_token! { pub fn left_parenthesis() -> TOKEN_LEFT_PARENTHESIS }

    get_node! { pub fn expression() -> 0 @ Expression }

    get_token! { pub fn right_parenthesis() -> TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! { #[from(NODE_LIST)] pub struct List => |self, formatter| {
    write!(formatter, "[")?;

    for expression in self.items() {
        use Expression as Ex;

        match expression {
            Ex::Parenthesis(variant) => write!(formatter, " {variant}"),
            Ex::List(variant) => write!(formatter, " {variant}"),
            Ex::AttributeSet(variant) => write!(formatter, " {variant}"),
            Ex::Path(variant) => write!(formatter, " {variant}"),
            Ex::Identifier(variant) => write!(formatter, " {variant}"),
            Ex::String(variant) => write!(formatter, " {variant}"),
            Ex::Island(variant) => write!(formatter, " {variant}"),
            Ex::Number(variant) => write!(formatter, " {variant}"),
            _ => write!(formatter, " ({expression})"),
        }?;
    }

    write!(formatter, " ]")
}}

impl List {
    get_token! { pub fn left_bracket() -> TOKEN_LEFT_BRACKET }

    get_node! { pub fn items() -> [Expression] }

    get_token! { pub fn right_bracket() -> TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE SET

node! { #[from(NODE_ATTRIBUTE_SET)] pub struct AttributeSet => |self, formatter| {
    write!(formatter, "{{")?;

    for inherit in self.inherits() {
        write!(formatter, " {inherit}")?;
    }

    for entry in self.entries() {
        write!(formatter, " {entry}")?;
    }

    write!(formatter, " }}")
}}

impl AttributeSet {
    get_token! { pub fn left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { pub fn inherits() -> [AttributeInherit] }

    get_node! { pub fn entries() -> [AttributeEntry] }

    get_token! { pub fn right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_ATTRIBUTE_INHERIT)] pub struct AttributeInherit => |self, formatter| write!(formatter, "{identifier};", identifier = self.identifier()) }

impl AttributeInherit {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE_ENTRY)] pub struct AttributeEntry => |self, formatter| write!(formatter, "{path} = {value};", path = self.path(), value = self.value()) }

impl AttributeEntry {
    get_node! { pub fn path() -> 0 @ AttributePath }

    get_node! { pub fn value() -> 0 @ Expression }

    get_token! { pub fn semicolon() -> TOKEN_SEMICOLON }
}

node! { #[from(NODE_ATTRIBUTE_PATH)] pub struct AttributePath => |self, formatter| {
    let mut identifiers = self.identifiers();

    write!(formatter, "{identifier}", identifier = identifiers.next().unwrap())?;

    for identifier in identifiers {
        write!(formatter, ".{identifier}")?;
    }

    Ok(())
}}

impl AttributePath {
    get_node! { pub fn identifiers() -> [Identifier] }
}

// BIND

node! { #[from(NODE_BIND)] pub struct Bind => |self, formatter| write!(formatter, "{identifier} @", identifier = self.identifier()) }

impl Bind {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn at() -> TOKEN_AT }
}

// USE

node! { #[from(NODE_USE)] pub struct Use => |self, formatter| {
    if let Some(bind) = self.bind() {
        write!(formatter, "{bind} ")?;
    }

    write!(
        formatter,
        "{left} ==> {right}",
        left = self.left_expression(),
        right = self.right_expression(),
    )
}}

impl Use {
    get_node! { pub fn bind() -> 0 @ ? Bind }

    get_node! { pub fn left_expression() -> 0 @ Expression }

    get_token! { pub fn right_long_arrow() -> TOKEN_EQUAL_EQUAL_MORE }

    get_node! { pub fn right_expression() -> 1 @ Expression }
}

// LAMBDA

node! { #[from(NODE_LAMBDA)] pub struct Lambda => |self, formatter| write!(formatter, "{parameter}: {expression}", parameter = self.parameter(), expression = self.expression()) }

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

node! { #[from(NODE_LAMBDA_PARAMETER_IDENTIFIER)] pub struct LambdaParameterIdentifier => |self, formatter| write!(formatter, "{identifier}", identifier = self.identifier()) }

impl LambdaParameterIdentifier {
    get_node! { pub fn identifier() -> 0 @ Identifier }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN)] pub struct LambdaParameterPattern => |self, formatter| {
    if let Some(bind) = self.bind() {
        write!(formatter, "{bind} ")?;
    }

    write!(formatter, "{{")?;

    for entry in self.entries() {
        write!(formatter, " {entry}")?;
    }

    write!(formatter, " }}")
}}

#[rustfmt::skip]
impl LambdaParameterPattern {
    get_node! { pub fn bind() -> 0 @ ? Bind }

    get_token! { pub fn left_curlybrace() -> TOKEN_LEFT_CURLYBRACE }

    get_node! { pub fn entries() -> [LambdaParameterPatternEntry] }

    get_token! { pub fn right_curlybrace() -> TOKEN_RIGHT_CURLYBRACE }
}

node! { #[from(NODE_LAMBDA_PARAMETER_PATTERN_ENTRY)] pub struct LambdaParameterPatternEntry => |self, formatter| {
    write!(formatter, "{identifier}", identifier = self.identifier())?;

    if let Some(default) = self.default() {
        write!(formatter, " ? {default}")?;
    }

    write!(formatter, ",")
}}

impl LambdaParameterPatternEntry {
    get_node! { pub fn identifier() -> 0 @ Identifier }

    get_token! { pub fn questionmark() -> ? TOKEN_QUESTIONMARK }

    get_node! { pub fn default() -> 1 @ ? Expression }
}

// APPLICATION

node! { #[from(NODE_APPLICATION)] pub struct Application => |self, formatter| write!(formatter, "{left} {right}", left = self.left_expression(), right = self.right_expression()) }

impl Application {
    get_node! { pub fn left_expression() -> 0 @ Expression }

    get_node! { pub fn right_expression() -> 1 @ Expression }
}

// PREFIX OPERATION

node! { #[from(NODE_PREFIX_OPERATION)] pub struct PrefixOperation => |self, formatter| write!(formatter, "{operator}{expression}", operator = self.operator(), expression = self.expression()) }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
    Swwallation, // Get it?
    Negation,

    Not,
}

impl fmt::Display for PrefixOperator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{operator}",
            operator = match self {
                Self::Swwallation => "+",
                Self::Negation => "-",

                Self::Not => "not ",
            }
        )
    }
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

node! { #[from(NODE_INFIX_OPERATION)] pub struct InfixOperation => |self, formatter| write!(formatter, "{left} {operator} {right}", left = self.left_expression(), operator = self.operator(), right = self.right_expression()) }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InfixOperator {
    Apply,
    Pipe,

    Concat,

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

impl fmt::Display for InfixOperator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{operator}",
            operator = match self {
                Self::Apply => "<|",
                Self::Pipe => "|>",

                Self::Concat => "++",

                Self::Override => "<==",
                Self::Update => "//",

                Self::Equal => "==",
                Self::NotEqual => "!=",
                Self::LessOrEqual => "<=",
                Self::Less => "<",
                Self::MoreOrEqual => ">=",
                Self::More => ">",
                Self::Implication => "->",

                Self::Addition => "+",
                Self::Negation => "-",
                Self::Multiplication => "*",
                Self::Power => "**",
                Self::Division => "/",

                Self::And => "and",
                Self::Or => "or",
            }
        )
    }
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

node! { #[from(NODE_INTERPOLATION)] pub struct Interpolation => |self, formatter| write!(formatter, "${{{expression}}}", expression = self.expression()) }

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

impl<T: Token> fmt::Display for InterpolationPart<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InterpolationPart::Content(path) => {
                write!(formatter, "{text}", text = path.syntax().text())
            },
            InterpolationPart::Interpolation(interpolation) => {
                write!(formatter, "{interpolation}")
            },
        }
    }
}

macro_rules! parted {
    (
        impl
        $name:ident { let content_kind = $content_kind:expr; type ContentToken = $content_token:ty; }
    ) => {
        impl $name {
            pub fn parts(&self) -> impl Iterator<Item = InterpolationPart<$content_token>> {
                self.syntax().children_with_tokens().map(|child| {
                    match child {
                        rowan::NodeOrToken::Token(token) => {
                            debug_assert_eq!(token.kind(), $content_kind);

                            InterpolationPart::Content(<$content_token>::cast(token).unwrap())
                        },

                        rowan::NodeOrToken::Node(node) => {
                            debug_assert_eq!(node.kind(), TOKEN_INTERPOLATION_START);

                            InterpolationPart::Interpolation(
                                Interpolation::cast(node.clone_subtree()).unwrap(),
                            )
                        },
                    }
                })
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let first = self.children_tokens_untyped().next().unwrap();
                if first.kind() != $content_kind {
                    write!(formatter, "{text}", text = first.text())?;
                }

                for part in self.parts() {
                    write!(formatter, "{part}")?;
                }

                let last = self.children_tokens_untyped().last().unwrap();
                if last.kind() != $content_kind {
                    write!(formatter, "{text}", text = last.text())?;
                }

                Ok(())
            }
        }
    };
}
// PATH, IDENTIFIER, STRING, ISLAND

node! { #[from(NODE_PATH)] pub struct Path; }

parted! {
    impl Path {
        let content_kind = TOKEN_PATH;
        type ContentToken = token::PathContent;
    }
}

node! { #[from(NODE_IDENTIFIER)] pub struct Identifier => |self, formatter| write!(formatter, "{value}", value = self.value()) }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdentifierValue {
    Simple(token::IdentifierSimple),
    Complex(IdentifierComplex),
}

impl fmt::Display for IdentifierValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple(simple) => write!(formatter, "{text}", text = simple.syntax().text()),
            Self::Complex(complex) => write!(formatter, "{complex}"),
        }
    }
}

node! { #[from(NODE_IDENTIFIER)] pub struct IdentifierComplex; }

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
            return IdentifierValue::Complex(IdentifierComplex(self.0.clone_subtree()));
        }

        unreachable!()
    }
}

node! { #[from(NODE_STRING)] pub struct String; }

parted! {
    impl String {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

node! { #[from(NODE_ISLAND)] pub struct Island; }

parted! {
    impl Island {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// NUMBER

node! { #[from(NODE_NUMBER)] pub struct Number => |self, formatter| write!(formatter, "{value}", value = self.value()) }

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumberValue {
    Integer(token::Integer),
    Float(token::Float),
}

impl fmt::Display for NumberValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(integer) => write!(formatter, "{value}", value = integer.value()),
            Self::Float(float) => write!(formatter, "{value}", value = float.value()),
        }
    }
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

node! { #[from(NODE_IF_ELSE)] pub struct IfElse => |self, formatter| {
    write!(formatter, "if {condition} then {trvke}", condition = self.condition(), trvke = self.true_expression())?;

    if let Some(nope) = self.false_expression() {
        write!(formatter, " else {nope}")?;
    }

    Ok(())
}}

impl IfElse {
    get_token! { pub fn if_() -> TOKEN_LITERAL_IF }

    get_node! { pub fn condition() -> 0 @ Expression }

    get_token! { pub fn then() -> TOKEN_LITERAL_THEN }

    get_node! { pub fn true_expression() -> 1 @ Expression }

    get_token! { pub fn else_() -> ? TOKEN_LITERAL_ELSE }

    get_node! { pub fn false_expression() -> 2 @ ? Expression }
}

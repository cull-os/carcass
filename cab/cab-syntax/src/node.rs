//! Typed [`Node`] definitions.
//!
//! [`Node`]: crate::Node
use std::{
    collections::VecDeque,
    mem,
    ops,
};

use cab_why::{
    IntoSpan,
    Report,
    Span,
};
use paste::paste;

use crate::{
    Kind::{
        self,
        *,
    },
    Token,
    red,
    token::{
        self,
        ContentPart,
    },
};

macro_rules! reffed {
    (
        $(#[$attribute:meta])*
        pub enum $name:ident $(<$($ident:ident $(: $bound:path)?),+>)? {
            $(
                $(#[$variant_attribute:meta])*
                $variant:ident($type:ty)
            ),* $(,)?
        }
    ) => {
        paste! {
            $(#[$attribute])*
            pub enum $name $(<$($ident $(: $bound)?),+>)? {
                $(
                    $(#[$variant_attribute])*
                    $variant($type)
                ),*
            }

            impl$(<$($ident $(: $bound)?),+>)? $name $(<$($ident),+>)? {
                pub fn as_ref(&self) -> [<$name Ref>]<'_$(, $($ident),+)?> {
                    match self {
                        $(
                            Self::$variant(v) => [<$name Ref>]::$variant(v)
                        ),*
                    }
                }
            }

            $(#[$attribute])*
            #[derive(Copy)]
            pub enum [<$name Ref>]<'a $(, $($ident $(: $bound)?),+)?> {
                $(
                    $(#[$variant_attribute])*
                    $variant(&'a $type)
                ),*
            }
        }
    };
}

macro_rules! node {
    (
        #[from($kind:ident)]
        struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        #[repr(transparent)]
        pub struct $name(red::Node);

        impl ops::Deref for $name {
            type Target = red::Node;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<'a> TryFrom<&'a red::Node> for &'a $name {
            type Error = ();

            fn try_from(node: &'a red::Node) -> Result<Self, ()> {
                if node.kind() != $kind {
                    return Err(());
                }

                // SAFETY: `node` is an immutable reference and Self is a &red::Node with
                // #[repr(transparent)].
                Ok(unsafe { mem::transmute::<&red::Node, Self>(node) })
            }
        }

        impl TryFrom<red::Node> for $name {
            type Error = ();

            fn try_from(node: red::Node) -> Result<Self, ()> {
                if node.kind() != $kind {
                    return Err(());
                }

                Ok(Self(node))
            }
        }

        impl $name {
            pub const KIND: Kind = $kind;
        }
    };

    (
        #[from($($variant:ident),* $(,)?)]
        enum $name:ident;
    ) => {
        reffed! {
            #[derive(Debug, Clone, PartialEq, Eq, Hash)]
            pub enum $name {
                $($variant($variant),)*
            }
        }

        impl ops::Deref for $name {
            type Target = red::Node;

            fn deref(&self) -> &Self::Target {
                match self {
                    $(Self::$variant(node) => &**node,)*
                }
            }
        }

        impl TryFrom<red::Node> for $name {
            type Error = ();

            fn try_from(node: red::Node) -> Result<Self, ()> {
                Ok(match node.kind() {
                    $($variant::KIND => Self::$variant($variant::try_from(node)?),)*
                    _ => return Err(()),
                })
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

                fn try_from(from: $name) -> Result<Self, ()> {
                    if let $name::$variant(node) = from {
                        Ok(node)
                    } else {
                        Err(())
                    }
                }
            }
        )*

        paste! {
            impl ops::Deref for [<$name Ref>]<'_> {
                type Target = red::Node;

                fn deref(&self) -> &Self::Target {
                    match self {
                        $(Self::$variant(node) => &**node,)*
                    }
                }
            }

            impl<'a> TryFrom<&'a red::Node> for [<$name Ref>]<'a> {
                type Error = ();

                fn try_from(node: &'a red::Node) -> Result<Self, ()> {
                    Ok(match node.kind() {
                        $($variant::KIND => Self::$variant(<&$variant>::try_from(node)?),)*
                        _ => return Err(()),
                    })
                }
            }

            $(
                impl<'a> From<&'a $variant> for [<$name Ref>]<'a> {
                    fn from(from: &'a $variant) -> Self {
                        Self::$variant(from)
                    }
                }

                impl<'a> TryFrom<[<$name Ref>]<'a>> for &'a $variant {
                    type Error = ();

                    fn try_from(from: [<$name Ref>]<'a>) -> Result<Self, ()> {
                        if let [<$name Ref>]::$variant(node) = from {
                            Ok(node)
                        } else {
                            Err(())
                        }
                    }
                }
            )*
        }
    };
}

macro_rules! get_token {
    ($name:ident -> $($skip:literal @)? Option<$kind:ident>) => {
        pub fn $name(&self) -> Option<&red::Token> {
            self.children_with_tokens()
                .filter_map(red::ElementRef::into_token)
                $(.skip($skip))?
                .find(|token| token.kind() == $kind)
        }
    };

    ($name:ident -> $($skip:literal @)? $kind:ident) => {
        pub fn $name(&self) -> &red::Token {
            self.children_with_tokens()
                .filter_map(red::ElementRef::into_token)
                $(.skip($skip))?
                .find(|token| token.kind() == $kind)
                .expect("node must have a token child")
        }
    };

    ($name:ident -> $($skip:literal @)? Option<$type:ty>) => {
        pub fn $name(&self) -> $type {
            self.children_with_tokens()
                .filter_map(red::ElementRef::into_token)
                $(.skip($skip))?
                .find_map(|token| <$type>::try_from(token).ok())
        }
    };

    ($name:ident -> $($skip:literal @)? $type:ty) => {
        pub fn $name(&self) -> $type {
            self.children_with_tokens()
                .filter_map(red::ElementRef::into_token)
                $(.skip($skip))?
                .find_map(|token| <$type>::try_from(token).ok())
                .expect("node must have a token child")
        }
    };
}

macro_rules! get_node {
    ($name:ident -> $($skip:literal @)? Option<$type:ty>) => {
        pub fn $name(&self) -> Option<$type> {
            self.children()
                .filter_map(|node| <$type>::try_from(node).ok())
                $(.skip($skip))?
                .next()
        }
    };

    ($name:ident -> $($skip:literal @)? $type:ty) => {
        pub fn $name(&self) -> $type {
            self.children()
                .filter_map(|node| <$type>::try_from(node).ok())
                $(.skip($skip))?
                .next()
                .expect("node must have a matching node child")
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
        Bind,
        Identifier,
        SString,
        Rune,
        Island,
        Integer,
        Float,
        If,
    )] enum Expression;
}

impl<'a> ExpressionRef<'a> {
    pub fn validate(self, to: &mut Vec<Report>) {
        match self {
            Self::Parenthesis(parenthesis) => parenthesis.validate(to),
            Self::List(list) => list.validate(to),
            Self::AttributeList(attribute_list) => attribute_list.validate(to),
            Self::PrefixOperation(operation) => operation.validate(to),
            Self::InfixOperation(operation) => operation.validate(to),
            Self::SuffixOperation(operation) => operation.validate(to),
            Self::Path(path) => path.validate(to),
            Self::Bind(bind) => bind.validate(to),
            Self::Identifier(identifier) => identifier.validate(to),
            Self::SString(string) => string.validate(to),
            Self::Rune(rune) => rune.validate(to),
            Self::Island(island) => island.validate(to),
            Self::If(if_else) => if_else.validate(to),

            Self::Error(_) | Self::Integer(_) | Self::Float(_) => {},
        }
    }

    pub fn same_items(self) -> impl Iterator<Item = ExpressionRef<'a>> {
        gen move {
            let mut expressions = VecDeque::from([self]);

            while let Some(expression) = expressions.pop_back() {
                match expression {
                    ExpressionRef::InfixOperation(operation) if let InfixOperator::Same = operation.operator() => {
                        expressions.push_front(operation.left());
                        expressions.push_front(operation.right());
                    },

                    ExpressionRef::SuffixOperation(operation) if let SuffixOperator::Same = operation.operator() => {
                        expressions.push_front(operation.left());
                    },

                    normal => yield normal,
                }
            }
        }
    }
}

// ERROR

node! { #[from(NODE_ERROR)] struct Error; }

// PARENTHESIS

node! { #[from(NODE_PARENTHESIS)] struct Parenthesis; }

impl Parenthesis {
    get_token! { token_parenthesis_left -> TOKEN_LEFT_PARENTHESIS }

    get_node! { expression -> Option<ExpressionRef<'_>> }

    get_token! { token_parenthesis_right -> Option<TOKEN_RIGHT_PARENTHESIS> }

    pub fn validate(&self, to: &mut Vec<Report>) {
        match self.expression() {
            Some(expression) => {
                expression.validate(to);
            },

            None => {
                to.push(Report::error("parenthesis without inner expression").primary(
                    Span::empty(self.token_parenthesis_left().span().end),
                    "expeted an expression here",
                ))
            },
        }

        if self.token_parenthesis_right().is_none() {
            to.push(
                Report::error("unclosed parenthesis")
                    .primary(Span::empty(self.span().end), "expected ')' here")
                    .secondary(self.token_parenthesis_left().span(), "unclosed '(' here"),
            );
        }
    }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

impl List {
    get_token! { token_bracket_left -> TOKEN_LEFT_BRACKET }

    get_node! { expression -> Option<ExpressionRef<'_>> }

    get_token! { token_bracket_right -> Option<TOKEN_RIGHT_BRACKET> }

    /// Returns all items of the list.
    pub fn items(&self) -> impl Iterator<Item = ExpressionRef<'_>> {
        self.expression().into_iter().flat_map(ExpressionRef::same_items)
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        if let Some(ExpressionRef::InfixOperation(operation)) = self.expression()
            && operation.operator() == InfixOperator::Sequence
        {
            to.push(
                Report::error("inner expression of list cannot be sequence")
                    .primary(operation.span(), "consider parenthesizing this"),
            );
        }

        for item in self.items() {
            item.validate(to);
        }

        if self.token_bracket_right().is_none() {
            to.push(
                Report::error("unclosed list")
                    .primary(Span::empty(self.span().end), "expected ']' here")
                    .secondary(self.token_bracket_left().span(), "unclosed '[' here"),
            );
        }
    }
}

// ATTRIBUTE LIST

node! { #[from(NODE_ATTRIBUTE_LIST)] struct AttributeList; }

impl AttributeList {
    get_token! { token_curlybrace_left -> TOKEN_LEFT_CURLYBRACE }

    get_node! { expression -> Option<ExpressionRef<'_>> }

    get_token! { token_curlybrace_right -> Option<TOKEN_RIGHT_CURLYBRACE> }

    /// Returns all entries of the attribute list.
    pub fn entries(&self) -> impl Iterator<Item = ExpressionRef<'_>> {
        self.expression().into_iter().flat_map(ExpressionRef::same_items)
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        for entry in self.entries() {
            match entry {
                ExpressionRef::InfixOperation(operation) if let InfixOperator::Sequence = operation.operator() => {
                    to.push(
                        Report::error("unexpected sequence operator inside attribute list").primary(
                            operation.span(),
                            "sequence operator has lower binding power and will consume everything",
                        ),
                    );
                },

                ExpressionRef::Identifier(identifier) => {
                    identifier.validate(to);
                },

                invalid => to.push(Report::error("invalid attribute").primary(invalid.span(), "here")),
            }
        }

        if self.token_curlybrace_right().is_none() {
            to.push(
                Report::error("unclosed attribute list")
                    .primary(Span::empty(self.span().end), "expected '}' here")
                    .secondary(self.token_curlybrace_left().span(), "unclosed '{' here"),
            );
        }
    }
}

// PREFIX OPERATION

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
    Swwallation, // Get it?
    Negation,

    Not,

    Try,
}

impl TryFrom<Kind> for PrefixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, ()> {
        Ok(match from {
            TOKEN_PLUS => Self::Swwallation,
            TOKEN_MINUS => Self::Negation,

            TOKEN_EXCLAMATIONMARK => Self::Not,

            TOKEN_QUESTIONMARK => Self::Try,

            _ => return Err(()),
        })
    }
}

impl PrefixOperator {
    /// Returns the binding power of this operator.
    pub fn binding_power(self) -> ((), u16) {
        match self {
            Self::Swwallation | Self::Negation => ((), 145),
            Self::Not => ((), 125),
            Self::Try => ((), 105),
        }
    }
}

node! { #[from(NODE_PREFIX_OPERATION)] struct PrefixOperation; }

impl PrefixOperation {
    get_node! { right -> 0 @ ExpressionRef<'_> }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> &red::Token {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find(|token| PrefixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> PrefixOperator {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find_map(|token| PrefixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        self.right().validate(to);
    }
}

// INFIX OPERATION

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InfixOperator {
    Same,
    Sequence,

    ImplicitApply,
    Apply,
    Pipe,

    Concat,
    Construct,

    Select,
    Update,

    LessOrEqual,
    Less,
    MoreOrEqual,
    More,

    Equal,
    NotEqual,

    And,
    Or,
    Implication,

    All,
    Any,

    Addition,
    Subtraction,
    Multiplication,
    Power,
    Division,

    Lambda,
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, ()> {
        Ok(match from {
            TOKEN_COMMA => Self::Same,
            TOKEN_SEMICOLON => Self::Sequence,

            kind if kind.is_argument() => Self::ImplicitApply,
            TOKEN_LESS_PIPE => Self::Apply,
            TOKEN_PIPE_MORE => Self::Pipe,

            TOKEN_PLUS_PLUS => Self::Concat,
            TOKEN_COLON => Self::Construct,

            TOKEN_PERIOD => Self::Select,
            TOKEN_SLASH_SLASH => Self::Update,

            TOKEN_LESS_EQUAL => Self::LessOrEqual,
            TOKEN_LESS => Self::Less,
            TOKEN_MORE_EQUAL => Self::MoreOrEqual,
            TOKEN_MORE => Self::More,

            TOKEN_EQUAL => Self::Equal,
            TOKEN_EXCLAMATION_EQUAL => Self::NotEqual,

            TOKEN_AMPERSAND_AMPERSAND => Self::And,
            TOKEN_PIPE_PIPE => Self::Or,
            TOKEN_MINUS_MORE => Self::Implication,

            TOKEN_AMPERSAND => Self::All,
            TOKEN_PIPE => Self::Any,

            TOKEN_PLUS => Self::Addition,
            TOKEN_MINUS => Self::Subtraction,
            TOKEN_ASTERISK => Self::Multiplication,
            TOKEN_CARET => Self::Power,
            TOKEN_SLASH => Self::Division,

            TOKEN_EQUAL_GREATER => Self::Lambda,

            _ => return Err(()),
        })
    }
}

impl InfixOperator {
    /// Returns the binding power of this operator.
    pub fn binding_power(self) -> (u16, u16) {
        match self {
            Self::Select => (185, 180),
            Self::ImplicitApply => (170, 175),

            Self::Concat => (160, 165),

            Self::Multiplication | Self::Division => (150, 155),
            Self::Power => (155, 150),

            // PrefixOperator::Swallation | PrefixOperator::Negation
            Self::Addition | Self::Subtraction => (130, 135),
            // PrefixOperator::Not
            Self::Update => (110, 115),

            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More /* | PrefixOperator::Try */ => {
                (100, 105)
            },

            Self::Construct => (95, 90),

            Self::And | Self::All => (85, 80),
            Self::Or | Self::Any => (75, 70),
            Self::Implication => (65, 60),

            Self::Pipe => (50, 55),
            Self::Apply => (55, 50),

            Self::Lambda => (45, 40),

            Self::Equal | Self::NotEqual => (35, 30),

            Self::Same => (25, 20),
            Self::Sequence => (15, 10),
        }
    }

    /// Whether if this operator actually owns a token. Not owning a token means
    /// that the operator doesn't actually "exist".
    pub fn is_token_owning(self) -> bool {
        self != Self::ImplicitApply
    }
}

node! { #[from(NODE_INFIX_OPERATION)] struct InfixOperation; }

impl InfixOperation {
    get_node! { left -> 0 @ ExpressionRef<'_> }

    get_node! { right -> 1 @ ExpressionRef<'_> }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> Option<&'_ red::Token> {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find(|token| InfixOperator::try_from(token.kind()).is_ok())
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> InfixOperator {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find_map(|token| InfixOperator::try_from(token.kind()).ok())
            .unwrap_or(InfixOperator::ImplicitApply)
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        let expressions = &[self.left(), self.right()];

        for expression in expressions {
            expression.validate(to);
        }

        let operator = self.operator();
        let (InfixOperator::Apply | InfixOperator::Pipe) = operator else {
            return;
        };

        for expression in expressions {
            if let ExpressionRef::InfixOperation(operation) = expression
                && let child_operator @ (InfixOperator::Apply | InfixOperator::Pipe) = operation.operator()
                && child_operator != operator
            {
                to.push(
                    Report::error("application and pipe operators do not associate")
                        .secondary(self.span(), "this")
                        .primary(operation.span(), "does not associate with this"),
                );
            }
        }
    }
}

// SUFFIX OPERATION

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SuffixOperator {
    Same,
    Sequence,
}

impl TryFrom<Kind> for SuffixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, ()> {
        match from {
            TOKEN_COMMA => Ok(Self::Same),
            TOKEN_SEMICOLON => Ok(Self::Sequence),

            _ => Err(()),
        }
    }
}

node! { #[from(NODE_SUFFIX_OPERATION)] struct SuffixOperation; }

impl SuffixOperation {
    get_node! { left -> 0 @ ExpressionRef<'_> }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> &'_ red::Token {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find(|token| SuffixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> SuffixOperator {
        self.children_with_tokens()
            .filter_map(red::ElementRef::into_token)
            .find_map(|token| SuffixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        self.left().validate(to);
    }
}

// INTERPOLATION

reffed! {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum InterpolatedPart<T: Token> {
        Delimiter(red::Token),
        Content(T),
        Interpolation(Interpolation),
    }
}

// TODO: Find ouy why the hell s/&self/self/ doesn't work. This should be Copy.
impl<T: Token> InterpolatedPartRef<'_, T> {
    pub fn is_delimiter(&self) -> bool {
        matches!(self, Self::Delimiter(_))
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Delimiter(delimiter) => delimiter.span(),
            Self::Content(content) => content.span(),
            Self::Interpolation(interpolation) => interpolation.span(),
        }
    }
}

node! { #[from(NODE_INTERPOLATION)] struct Interpolation; }

impl Interpolation {
    get_token! { interpolation_token_start -> TOKEN_INTERPOLATION_START }

    get_node! { expression -> 0 @ ExpressionRef<'_> }

    get_token! { interpolation_token_end -> Option<TOKEN_INTERPOLATION_END> }

    pub fn validate(&self, to: &mut Vec<Report>) {
        self.expression().validate(to);
    }
}

macro_rules! parted {
    ($content_type:ty) => {
        pub fn parts(&self) -> impl Iterator<Item = InterpolatedPartRef<'_, $content_type>> {
            self.children_with_tokens().map(|child| {
                match child {
                    red::ElementRef::Token(token) => {
                        if let Ok(token) = <&$content_type>::try_from(token) {
                            InterpolatedPartRef::Content(token)
                        } else {
                            InterpolatedPartRef::Delimiter(token)
                        }
                    },

                    red::ElementRef::Node(node) => {
                        InterpolatedPartRef::Interpolation(
                            <&Interpolation>::try_from(node)
                                .expect("child node of a parted element wasn't an interpolation"),
                        )
                    },
                }
            })
        }
    };
}

// PATH

node! { #[from(NODE_PATH)] struct Path; }

impl Path {
    parted!(token::PathContent);

    pub fn validate(&self, to: &mut Vec<Report>) {
        for part in self.parts() {
            if let InterpolatedPartRef::Interpolation(interpolation) = part {
                interpolation.validate(to);
            }
        }
    }
}

// BIND

node! { #[from(NODE_BIND)] struct Bind; }

impl Bind {
    get_token! { token_at -> TOKEN_AT }

    get_node! { identifier -> ExpressionRef<'_> }

    pub fn validate(&self, to: &mut Vec<Report>) {
        let identifier = self.identifier();

        if let ExpressionRef::Identifier(identifier) = identifier {
            identifier.validate(to);
        } else {
            to.push(Report::error("invalid bind").primary(
                identifier.span(),
                format!("expected an identifier, not {kind}", kind = identifier.kind()),
            ));
        }
    }
}

// IDENTIFIER

node! { #[from(NODE_IDENTIFIER)] struct IdentifierQuoted; }

impl IdentifierQuoted {
    parted!(token::Content);

    pub fn validate(&self, to: &mut Vec<Report>) {
        let mut report = Report::error("invalid identifier");
        let mut reported_control_character = false;

        for part in self.parts() {
            match part {
                InterpolatedPartRef::Content(content) => {
                    content.parts(&mut report).count();

                    let text = content.text();

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(content.span(), "contains control characters");
                        report.push_help(
                            "quoted identifiers cannot contain control characters (non-escaped newlines, tabs, ...)",
                        );
                    }
                },

                InterpolatedPartRef::Interpolation(interpolation) => {
                    interpolation.validate(to);
                },

                _ => {},
            }
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

reffed! {
    /// An identifier value.
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub enum IdentifierValue {
        /// A plain identifier backed by a [`token::Identifier`].
        Plain(token::Identifier),
        /// A quoted identifier backed by a [`IdentifierQuoted`].
        Quoted(IdentifierQuoted),
    }
}

node! { #[from(NODE_IDENTIFIER)] struct Identifier; }

impl Identifier {
    /// Returns the value of this identifier. A value may either be a
    /// [`token::Identifier`] or a [`IdentifierQuoted`].
    pub fn value(&self) -> IdentifierValueRef<'_> {
        let Some(first_token) = self.first_token() else {
            unreachable!()
        };

        assert!(!first_token.kind().is_trivia());

        if let Ok(token) = <&token::Identifier>::try_from(first_token) {
            return IdentifierValueRef::Plain(token);
        }

        if let Ok(quoted) = <&IdentifierQuoted>::try_from(&**self) {
            return IdentifierValueRef::Quoted(quoted);
        }

        unreachable!("identifier node did not have an identifier or identifier starter token")
    }

    pub fn validate(&self, to: &mut Vec<Report>) {
        if let IdentifierValueRef::Quoted(quoted) = self.value() {
            quoted.validate(to);
        }
    }
}

// STRING

node! { #[from(NODE_STRING)] struct SString; }

impl SString {
    parted!(token::Content);

    // What a behemoth. And the sad part is I can't figure out a way to make this
    // simpler.
    pub fn validate(&self, to: &mut Vec<Report>) {
        let mut report = Report::error("invalid string");
        let mut reported_mixed_indentation = false;

        let mut parts = self
            .parts()
            .scan(0, |index, part| {
                let value = *index;

                match part {
                    InterpolatedPartRef::Delimiter(_) => {},

                    _ => *index += 1,
                }

                Some((value, part))
            })
            .peekable();

        let mut string_is_multiline = false;
        let mut string_first_line_span = None;
        let mut string_last_line_span = None;

        let mut indentation: Option<char> = None;

        let mut previous_span = None;
        while let Some((part_index, part)) = parts.next() {
            let mut part_is_multiline = true;
            let part_is_first = part_index == 0;
            let part_is_last = parts.peek().is_none_or(|(_, part)| part.is_delimiter());

            match &part {
                InterpolatedPartRef::Interpolation(interpolation) => {
                    interpolation.validate(to);

                    let span = interpolation.span();

                    if part_is_first {
                        string_first_line_span = Some(span);
                    } else if part_is_last {
                        string_last_line_span = Some(span);
                    }
                },

                InterpolatedPartRef::Content(content) => {
                    content.parts(&mut report).count();

                    let text = content.text();

                    let mut lines = text.split('\n').enumerate().peekable();
                    while let Some((line_index, line)) = lines.next() {
                        let line_is_first = line_index == 0;
                        let line_is_last = lines.peek().is_none();

                        let line_is_firstest = part_is_first && line_is_first;
                        let line_is_lastest = part_is_last && line_is_last;

                        if line_is_first && line_is_last {
                            part_is_multiline = false;
                        }

                        if line_is_firstest {
                            if !line.trim().is_empty() {
                                string_first_line_span = Some(Span::at(content.span().start, line.trim_end().len()));
                            } else if text.trim().is_empty()
                                && let Some((_, part)) = parts.peek()
                                && !part.is_delimiter()
                            {
                                string_first_line_span = Some(part.span());
                            }
                        } else if line_is_lastest {
                            if !line.trim().is_empty() {
                                let last_line_length = line.trim_start().len();

                                string_last_line_span = Some(Span::at_end(content.span().end, last_line_length));
                            } else if !part_is_multiline && let Some(span) = previous_span {
                                string_last_line_span = Some(span);
                            }
                        }

                        #[allow(clippy::nonminimal_bool)]
                        if
                        // Ignore firstest and lastest lines.
                        !(line_is_firstest || line_is_lastest)
                            // Ignore lines right after an interpolation end.
                            && !(previous_span.is_some() && line_is_first)
                        {
                            for c in line.chars() {
                                if !c.is_whitespace() {
                                    break;
                                }

                                let Some(indentation) = indentation else {
                                    indentation = Some(c);
                                    continue;
                                };

                                if !reported_mixed_indentation && indentation != c {
                                    reported_mixed_indentation = true;
                                    report.push_primary(
                                        self.span(),
                                        "strings cannot mix different kinds of whitespace in indentation",
                                    );
                                }
                            }
                        }
                    }
                },

                _ => {},
            }

            if !part.is_delimiter() {
                previous_span = Some(part.span());
            }

            if part_is_multiline {
                string_is_multiline = true;
            }
        }

        if string_is_multiline {
            for span in [string_first_line_span, string_last_line_span].into_iter().flatten() {
                report.push_primary(span, "multiline strings' first and last lines must be empty");
            }
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

// RUNE

node! { #[from(NODE_RUNE)] struct Rune; }

impl Rune {
    parted!(token::Content);

    pub fn validate(&self, to: &mut Vec<Report>) {
        let mut report = Report::error("invalid rune");
        let mut reported_invalid_length = false;
        let mut reported_control_character = false;
        let mut reported_interpolation = false;

        let mut got_content = false;

        for part in self.parts() {
            match part {
                InterpolatedPartRef::Content(content) => {
                    let text = content.text();

                    if !reported_invalid_length && {
                        let mut parts = content.parts(&mut report);

                        match (parts.next(), parts.next()) {
                            (Some(ContentPart::Literal(text)), None) if text.chars().count() == 1 => false,
                            (Some(ContentPart::Escape(_)), None) => false,

                            _ => true,
                        }
                    } {
                        reported_invalid_length = true;
                        report.push_primary(content.span(), "invalid rune literal length");
                    }

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(
                            content.span(),
                            "runes cannot contain control characters (non-escaped newlines, tabs, ...)",
                        );
                    }
                },

                InterpolatedPartRef::Interpolation(interpolation) if !reported_interpolation => {
                    reported_interpolation = true;
                    report.push_primary(interpolation.span(), "runes cannot contain interpolation");
                },

                _ => continue,
            }

            got_content = true;
        }

        if !got_content {
            report.push_primary(self.span(), "runes cannot be empty");
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

// ISLAND

node! { #[from(NODE_ISLAND)] struct Island; }

impl Island {
    parted!(token::Content);

    pub fn validate(&self, to: &mut Vec<Report>) {
        let mut report = Report::error("invalid island");
        let mut reported_control_character = false;

        for part in self.parts() {
            match part {
                InterpolatedPartRef::Content(content) => {
                    content.parts(&mut report).count();

                    let text = content.text();

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(content.span(), "here");
                        report.push_tip("islands cannot contain control characters (non-escaped newlines, tabs, ...)");
                    }
                },

                InterpolatedPartRef::Interpolation(interpolation) => {
                    interpolation.validate(to);
                },

                _ => {},
            }
        }

        if !report.is_empty() {
            to.push(report)
        }
    }
}

// INTEGER

node! { #[from(NODE_INTEGER)] struct Integer; }

impl Integer {
    get_token! { token_integer -> &token::Integer }

    pub fn value(&self) -> num::BigInt {
        self.token_integer().value()
    }
}

// FLOAT

node! { #[from(NODE_FLOAT)] struct Float; }

impl Float {
    get_token! { token_float -> &token::Float }

    pub fn value(&self) -> f64 {
        self.token_float().value()
    }
}

// IF

node! { #[from(NODE_IF)] struct If; }

impl If {
    get_token! { token_if -> TOKEN_LITERAL_IF }

    get_node! { condition -> 0 @ ExpressionRef<'_> }

    get_token! { token_then -> TOKEN_LITERAL_THEN }

    get_node! { consequence -> 1 @ ExpressionRef<'_> }

    get_token! { token_else -> Option<TOKEN_LITERAL_ELSE> }

    get_node! { alternative -> 2 @ ExpressionRef<'_> }

    pub fn validate(&self, to: &mut Vec<Report>) {
        self.condition().validate(to);
        self.consequence().validate(to);
        self.alternative().validate(to);
    }
}

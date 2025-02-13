//! [`Node`] definitions.
use std::{
    collections::VecDeque,
    ops::{
        self,
        Deref as _,
    },
};

use cab_report::Report;
use enumset::EnumSet;
use rowan::ast::AstNode as _;
use smallvec::SmallVec;
use static_assertions::assert_obj_safe;

use crate::{
    Kind::{
        self,
        *,
    },
    RedNode,
    RedNodeOrToken,
    RedToken,
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
///    IfThen as if_then => { unimplemented!() },
///    Identifier as identifier => { unimplemented!() },
///    else => unimplemented!(),
/// }
/// ```
#[doc(inline)]
pub use crate::__node_match as r#match;

assert_obj_safe!(Node);

/// A typed AST node. Implementors will usually have methods to make accessing
/// children elements and attributes related to the node simpler.
pub trait Node: rowan::ast::AstNode<Language = Language> + ops::Deref<Target = RedNode> {
    /// Returns its inherent kind, returning None if it is a node that can be
    /// created from multiple different kinds.
    fn kind() -> EnumSet<Kind>
    where
        Self: Sized;

    /// Validates the node appending reports to the provided [`Vec`]. If there
    /// are no reports appended the node is valid.
    fn validate(&self, _to: &mut Vec<Report<'_>>) {}

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
    fn token_untyped(&self, kind: Kind) -> Option<RedToken>
    where
        Self: Sized,
    {
        self.children_tokens_untyped().find(|token| token.kind() == kind)
    }

    /// Returns all immediate children tokens that can be cast to the given
    /// typed token.
    fn children_tokens<T: Token>(&self) -> impl Iterator<Item = T>
    where
        Self: Sized,
    {
        self.children_with_tokens()
            .filter_map(RedNodeOrToken::into_token)
            .filter_map(T::cast)
    }

    /// Returns all immediate children tokens.
    fn children_tokens_untyped(&self) -> impl Iterator<Item = RedToken>
    where
        Self: Sized,
    {
        self.children_with_tokens().filter_map(RedNodeOrToken::into_token)
    }
}

macro_rules! node {
    (
        #[from($kind:ident)]
        struct $name:ident;

        $($items:item)*
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) RowanNode);

        impl rowan::ast::AstNode for $name {
            type Language = Language;

            fn can_cast(kind: Kind) -> bool {
                kind == Self::KIND
            }

            fn cast(token: RowanNode) -> Option<Self> {
                Self::can_cast(token.kind()).then_some(Self(token))
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
            fn kind() -> EnumSet<Kind> {
                Self::KIND.into()
            }

            $($items)*
        }

        impl $name {
            /// The syntax kind this node can be cast from.
            pub const KIND: Kind = $kind;
        }
    };

    (
        #[from($($variant:ident),* $(,)?)]
        enum $name:ident;

        $($items:item)*
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

            fn cast(token: RowanNode) -> Option<Self> {
                match token.kind() {
                    $($variant::KIND => Some(Self::$variant($variant(token))),)*
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

        impl Node for $name {
            fn kind() -> EnumSet<Kind> {
                $($variant::KIND)|*
            }

            $($items)*
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
        Rune,
        Island,
        Number,
        IfThen,
        IfIs,
    )] enum Expression;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        match self {
            Self::Error(error) => error.validate(to),
            Self::Parenthesis(parenthesis) => parenthesis.validate(to),
            Self::List(list) => list.validate(to),
            Self::AttributeList(attribute_list) => attribute_list.validate(to),
            Self::PrefixOperation(operation) => operation.validate(to),
            Self::InfixOperation(operation) => operation.validate(to),
            Self::SuffixOperation(operation) => operation.validate(to),
            Self::Path(path) => path.validate(to),
            Self::Identifier(identifier) => identifier.validate(to),
            Self::SString(string) => string.validate(to),
            Self::Rune(rune) => rune.validate(to),
            Self::Island(island) => island.validate(to),
            Self::Number(number) => number.validate(to),
            Self::IfThen(if_else) => if_else.validate(to),
            Self::IfIs(if_is) => if_is.validate(to),
        }
    }
}

impl Expression {
    fn validate_pattern(&self, to: &mut Vec<Report<'_>>) {
        match self {
            Self::Parenthesis(parenthesis) => {
                parenthesis.expression().validate_pattern(to);
            },

            Self::List(list) => {
                for item in list.items() {
                    item.validate_pattern(to);
                }
            },

            // All attribute lists are valid patterns.
            // TODO: Add warns for right-side expression that
            // statically never use the attribute list made the scope.
            Self::AttributeList(attribute_list) => attribute_list.validate(to),

            Self::InfixOperation(operation)
                if let InfixOperator::ImplicitApply | InfixOperator::Apply = operation.operator() =>
            {
                operation.left().validate(to);
                operation.right().validate_pattern(to);
            },

            Self::InfixOperation(operation) if let InfixOperator::Pipe = operation.operator() => {
                operation.left().validate_pattern(to);
                operation.right().validate(to);
            },

            Self::InfixOperation(operation) if let InfixOperator::Construct = operation.operator() => {
                operation.left().validate_pattern(to);
                operation.right().validate_pattern(to);
            },

            Self::InfixOperation(operation) if let InfixOperator::Select = operation.operator() => {
                match operation.left() {
                    Expression::Identifier(identifier) => {
                        identifier.validate(to);
                    },

                    invalid => {
                        to.push(
                            Report::error("invalid select pattern")
                                .primary(invalid.text_range().into(), "left operand must be an identifier"),
                        )
                    },
                }

                operation.right().validate_pattern(to);
            },

            Self::InfixOperation(operation) if let InfixOperator::All | InfixOperator::Any = operation.operator() => {
                operation.left().validate_pattern(to);
                operation.right().validate_pattern(to);
            },

            Self::InfixOperation(operation) if let InfixOperator::This = operation.operator() => {
                operation.validate_left(to);
                operation.right().validate_pattern(to);
            },

            _ => {
                let mut report = Report::error("arithmetic patterns cannot have multiple binds");

                let mut bind_ranges = SmallVec::new();
                self.validate_pattern_arithmetic(&mut bind_ranges, to);
                for bind_range in bind_ranges {
                    report.push_primary(bind_range.into(), "bind");
                }

                if report.labels.len() > 1 {
                    to.push(report);
                }
            },
        }
    }

    fn validate_pattern_arithmetic(
        &self,
        bind_ranges: &mut SmallVec<rowan::TextRange, 4>,
        reports: &mut Vec<Report<'_>>,
    ) {
        match self {
            Expression::Parenthesis(parenthesis) => {
                parenthesis.expression().validate_pattern(reports);
            },

            Expression::InfixOperation(operation)
                if let InfixOperator::Addition
                | InfixOperator::Subtraction
                | InfixOperator::Multiplication
                | InfixOperator::Power
                | InfixOperator::Division = operation.operator() =>
            {
                operation.left().validate_pattern_arithmetic(bind_ranges, reports);
                operation.right().validate_pattern_arithmetic(bind_ranges, reports);
            },

            Expression::InfixOperation(_) => {
                reports.push(
                    Report::error("non-arithmetic infix operators are not valid patterns")
                        .primary(self.text_range().into(), "here"),
                );
            },

            Expression::Identifier(identifier) => {
                identifier.validate(reports);
                bind_ranges.push(identifier.text_range());
            },

            Expression::SString(string) => {
                string.validate(reports);
            },

            Expression::Number(number) => {
                number.validate(reports);
            },

            _ => {
                reports.push(Report::error("invalid pattern").primary(
                    self.text_range().into(),
                    format!("{kind} is not a valid pattern element", kind = self.kind()),
                ));
            },
        }
    }

    fn same_items(self) -> impl Iterator<Item = Expression> {
        gen {
            let mut expressions = VecDeque::from([self]);

            while let Some(expression) = expressions.pop_back() {
                match expression {
                    Expression::InfixOperation(operation) if let InfixOperator::Same = operation.operator() => {
                        expressions.push_front(operation.left());
                        expressions.push_front(operation.right());
                    },

                    Expression::SuffixOperation(operation) if let SuffixOperator::Same = operation.operator() => {
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

node! {
    #[from(NODE_PARENTHESIS)] struct Parenthesis;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.expression().validate(to);
    }
}

impl Parenthesis {
    get_token! { left_parenthesis_token -> TOKEN_LEFT_PARENTHESIS }

    get_node! { expression -> 0 @ Expression }

    get_token! { right_parenthesis_token -> ? TOKEN_RIGHT_PARENTHESIS }
}

// LIST

node! {
   #[from(NODE_LIST)] struct List;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        if let Some(Expression::InfixOperation(operation)) = self.expression()
            && operation.operator() == InfixOperator::Sequence
        {
            to.push(
                Report::error("inner expression of list cannot be sequence")
                    .primary(operation.text_range().into(), "consider parenthesizing this")
            );
        }

        for item in self.items() {
            item.validate(to);
        }
    }
}

#[rustfmt::skip]
impl List {
    get_token! { left_bracket_token -> TOKEN_LEFT_BRACKET }

    get_node! { expression -> 0 @ ? Expression }

    /// Returns all items of the list.
    pub fn items(&self) -> impl Iterator<Item = Expression> {
        self.expression().into_iter().flat_map(Expression::same_items)
    }

    get_token! { right_bracket_token -> ? TOKEN_RIGHT_BRACKET }
}

// ATTRIBUTE LIST

node! {
    #[from(NODE_ATTRIBUTE_LIST)] struct AttributeList;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        for entry in self.entries() {
            match entry {
                Expression::InfixOperation(operation) if let InfixOperator::Sequence = operation.operator() => {
                    to.push(
                        Report::error("unexpected sequence operator inside attribute list")
                            .primary(operation.text_range().into(), "sequence operator has lower binding power and will consume everything")
                    );
                }

                Expression::InfixOperation(operation) if let InfixOperator::Bind = operation.operator() => {
                    operation.validate(to);
                },

                Expression::Identifier(identifier) => {
                    identifier.validate(to);
                },

                invalid => {
                    to.push(Report::error(
                        "invalid attribute").primary(
                        invalid.text_range().into(), "here"
                    ))
                }
            }
        }
    }
}

#[rustfmt::skip]
impl AttributeList {
    get_token! { left_curlybrace_token -> TOKEN_LEFT_CURLYBRACE }

    get_node! { expression -> 0 @ ? Expression }

    /// Returns all entries of the attribute list.
    pub fn entries(&self) -> impl Iterator<Item = Expression> {
        self.expression().into_iter().flat_map(Expression::same_items)
    }

    get_token! { right_curlybrace_token -> ? TOKEN_RIGHT_CURLYBRACE }
}

// PREFIX OPERATION

node! {
    #[from(NODE_PREFIX_OPERATION)] struct PrefixOperation;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.right().validate(to);
    }
}

#[rustfmt::skip]
impl PrefixOperation {
    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> RedToken {
        self.children_tokens_untyped()
            .find(|token| PrefixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> PrefixOperator {
        self.children_tokens_untyped()
            .find_map(|token| PrefixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    get_node! { right -> 0 @ Expression }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrefixOperator {
    Swwallation, // Get it?
    Negation,

    Not,

    Try,
}

impl TryFrom<Kind> for PrefixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
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
            Self::Swwallation | Self::Negation => ((), 165),
            Self::Not => ((), 145),
            Self::Try => ((), 125),
        }
    }
}

// INFIX OPERATION

node! {
    #[from(NODE_INFIX_OPERATION)] struct InfixOperation;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        match self.operator() {
            InfixOperator::Same => {
                let mut same_operator = None;

                for item in Expression::InfixOperation(self.clone()).same_items() {
                    if let Expression::InfixOperation(operation) = &item
                        && let operator @ (InfixOperator::Lambda | InfixOperator::Bind) = operation.operator()
                    {
                        let same_operator = same_operator.get_or_insert(operator);

                        if *same_operator != operator {
                            to.push(
                                Report::error("invalid same-operand")
                                    .primary(item.text_range().into(), "operand")
                                    .help("all same-operands must be of matching type: either all lambdas or all binds")
                            );
                            continue;
                        }

                        operation.validate(to);
                    } else {
                        to.push(
                            Report::error("invalid same-operand")
                                .primary(item.text_range().into(), "operand")
                                .help("all same-operands must either be lambdas or binds")
                        );
                    }
                }
            },

            InfixOperator::This => {
                self.validate_left(to);
                self.right().validate(to);
            }

            InfixOperator::Lambda | InfixOperator::Bind => {
                self.left().validate_pattern(to);
                self.right().validate(to);
            },

            operator => {
                let expressions = &[self.left(), self.right()];

                for expression in expressions {
                    expression.validate(to);
                }

                let (InfixOperator::Apply | InfixOperator::Pipe) = operator else { return; };

                for expression in expressions {
                    if let Expression::InfixOperation(operation) = expression
                        && let child_operator @ (InfixOperator::Apply | InfixOperator::Pipe) = operation.operator()
                        && child_operator != operator
                    {
                        to.push(
                            Report::error("application and pipe operators do not associate")
                                .secondary(self.text_range().into(), "this")
                                .primary(operation.text_range().into(), "does not associate with this")
                        );
                    }
                }
            },
        }
    }
}

#[rustfmt::skip]
impl InfixOperation {
    get_node! { left -> 0 @ Expression }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> Option<RedToken> {
        self.children_tokens_untyped()
            .find(|token| InfixOperator::try_from(token.kind()).is_ok())
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> InfixOperator {
        self.children_tokens_untyped()
            .find_map(|token| InfixOperator::try_from(token.kind()).ok())
            .unwrap_or(InfixOperator::ImplicitApply)
    }

    get_node! { right -> 1 @ Expression }

    /// Asserts that this node is a this-expression and validates the left
    /// expression.
    pub fn validate_left(&self, to: &mut Vec<Report<'_>>) {
        assert_eq!(self.operator(), InfixOperator::This);

        match self.left() {
            Expression::Identifier(identifier) => {
                   identifier.validate(to);
            },

            invalid => {
                to.push(
                    Report::error("left operand of a this-expression must be an identifier")
                        .primary(invalid.text_range().into(), "left operand")
                );
            }
        }
    }
}

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

    This,
    Lambda,
    Bind,
}

impl TryFrom<Kind> for InfixOperator {
    type Error = ();

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
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

            TOKEN_EQUAL_EQUAL => Self::Equal,
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

            TOKEN_AT => Self::This,
            TOKEN_EQUAL_GREATER => Self::Lambda,
            TOKEN_COLON_EQUAL => Self::Bind,

            _ => return Err(()),
        })
    }
}

impl InfixOperator {
    /// Returns the binding power of this operator.
    pub fn binding_power(self) -> (u16, u16) {
        match self {
            Self::Select => (205, 200),
            Self::ImplicitApply => (190, 195),

            Self::Concat => (180, 185),

            Self::Multiplication | Self::Division => (170, 175),
            Self::Power => (175, 170),

            // PrefixOperator::Swallation | PrefixOperator::Negation
            Self::Addition | Self::Subtraction => (150, 155),
            // PrefixOperator::Not
            Self::Update => (130, 135),

            Self::LessOrEqual | Self::Less | Self::MoreOrEqual | Self::More /* | PrefixOperator::Try */ => {
                (120, 125)
            },

            Self::Construct => (115, 110),

            Self::Equal | Self::NotEqual => (105, 100),

            Self::And | Self::All => (95, 90),
            Self::Or | Self::Any => (85, 80),
            Self::Implication => (75, 70),

            Self::Pipe => (60, 65),
            Self::Apply => (65, 60),

            Self::This => (55, 50),
            Self::Lambda => (45, 40),
            Self::Bind => (35, 30),

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

// SUFFIX OPERATION

node! {
    #[from(NODE_SUFFIX_OPERATION)] struct SuffixOperation;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.left().validate(to);
    }
}

impl SuffixOperation {
    get_node! { left -> 0 @ Expression }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> RedToken {
        self.children_tokens_untyped()
            .find(|token| SuffixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> SuffixOperator {
        self.children_tokens_untyped()
            .find_map(|token| SuffixOperator::try_from(token.kind()).ok())
            .unwrap()
    }
}

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

// INTERPOLATION

node! {
    #[from(NODE_INTERPOLATION)] struct Interpolation;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.expression().validate(to);
    }
}

impl Interpolation {
    get_token! { interpolation_start_token -> TOKEN_INTERPOLATION_START }

    get_node! { expression -> 0 @ Expression }

    get_token! { interpolation_end_token -> ? TOKEN_INTERPOLATION_END }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterpolatedPart<T: Token> {
    Delimiter(RedToken),
    Content(T),
    Interpolation(Interpolation),
}

impl<T: Token> InterpolatedPart<T> {
    pub fn is_delimiter(&self) -> bool {
        matches!(self, Self::Delimiter(_))
    }

    pub fn text_range(&self) -> rowan::TextRange {
        match self {
            InterpolatedPart::Delimiter(delimiter) => delimiter.text_range(),
            InterpolatedPart::Content(content) => content.text_range(),
            InterpolatedPart::Interpolation(interpolation) => interpolation.text_range(),
        }
    }
}

macro_rules! interpolated {
    (impl $name:ident { let content_kind = $content_kind:expr; type ContentToken = $content_token:ty; }) => {
        impl $name {
            pub fn parts(&self) -> impl Iterator<Item = InterpolatedPart<$content_token>> {
                self.children_with_tokens().map(|child| {
                    match child {
                        rowan::NodeOrToken::Token(token) => {
                            if token.kind() == $content_kind {
                                InterpolatedPart::Content(<$content_token>::cast(token).unwrap())
                            } else {
                                InterpolatedPart::Delimiter(token)
                            }
                        },

                        rowan::NodeOrToken::Node(node) => {
                            assert_eq!(node.kind(), NODE_INTERPOLATION);

                            InterpolatedPart::Interpolation(Interpolation::cast(node.clone()).unwrap())
                        },
                    }
                })
            }
        }
    };
}

// PATH

node! {
    #[from(NODE_PATH)] struct Path;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        for part in self.parts() {
            if let InterpolatedPart::Interpolation(interpolation) = part {
                interpolation.validate(to);
            }
        }
    }
}

interpolated! {
    impl Path {
        let content_kind = TOKEN_PATH;
        type ContentToken = token::Path;
    }
}

// IDENTIFIER

node! {
    #[from(NODE_IDENTIFIER)] struct Identifier;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        if let IdentifierValue::Quoted(quoted) = self.value() {
            quoted.validate(to);
        }
    }
}

impl Identifier {
    /// Returns the value of this identifier. A value may either be a
    /// [`token::Identifier`] or a quoted stringlike.
    pub fn value(&self) -> IdentifierValue {
        if let Some(token) = self.token() {
            return IdentifierValue::Plain(token);
        }

        if self.token_untyped(TOKEN_IDENTIFIER_START).is_some() {
            return IdentifierValue::Quoted(IdentifierQuoted(self.deref().clone()));
        }

        unreachable!()
    }
}

/// An identifier value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdentifierValue {
    /// A plain identifier backed by a [`token::Identifier`].
    Plain(token::Identifier),
    /// A quoted identifier backed by a stringlike [`IdentifierQuoted`].
    Quoted(IdentifierQuoted),
}

node! {
    #[from(NODE_IDENTIFIER)] struct IdentifierQuoted;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid identifier");
        let mut reported_control_character = false;

        for part in self.parts() {
            match part {
                InterpolatedPart::Content(content) => {
                    content.validate_escapes(&mut report);

                    let text = content.text();

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(content.text_range().into(), "contains control characters");
                        report.push_help("quoted identifiers cannot contain control characters (non-escaped newlines, tabs, ...)");
                    }
                },

                InterpolatedPart::Interpolation(interpolation) => {
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

interpolated! {
    impl IdentifierQuoted {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// STRING

node! {
    #[from(NODE_STRING)] struct SString;

    // What a behemoth. And the sad part is I can't figure out a way to make this simpler.
    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid string");
        let mut reported_mixed_indentation = false;

        let mut parts = self
            .parts()
            .scan(0, |index, part| {
                let value = *index;

                match part {
                    InterpolatedPart::Delimiter(_) => {},

                    _ => *index += 1,
                }

                Some((value, part))
            })
            .peekable();

        let mut string_is_multiline = false;
        let mut string_first_line_range = None;
        let mut string_last_line_range = None;

        let mut indentation: Option<char> = None;

        let mut previous_part_is_not_delimiter_range = None;
        while let Some((part_index, part)) = parts.next() {
            let mut part_is_multiline = true;
            let part_is_first = part_index == 0;
            let part_is_last = parts.peek().is_none_or(|(_, part)| part.is_delimiter());

            match &part {
                InterpolatedPart::Interpolation(interpolation) => {
                    interpolation.validate(to);

                    let range = interpolation.text_range();

                    if part_is_first {
                        string_first_line_range = Some(range);
                    } else if part_is_last {
                        string_last_line_range = Some(range);
                    }
                },

                InterpolatedPart::Content(content) => {
                    content.validate_escapes(&mut report);

                    let text = content.text();

                    let mut lines = text.split('\n').enumerate().peekable();
                    while let Some((line_index, line)) = lines.next()  {
                        let line_is_first = line_index == 0;
                        let line_is_last = lines.peek().is_none();

                        let line_is_firstest = part_is_first && line_is_first;
                        let line_is_lastest = part_is_last && line_is_last;

                        if line_is_first && line_is_last {
                            part_is_multiline = false;
                        }

                        if line_is_firstest {
                            if !line.trim().is_empty() {
                                string_first_line_range = Some(rowan::TextRange::at(
                                    content.text_range().start(),
                                    rowan::TextSize::new(line.trim_end().len() as u32),
                                ));
                            } else if text.trim().is_empty()
                                && let Some((_, part)) = parts.peek()
                                && !part.is_delimiter() {
                                string_first_line_range = Some(part.text_range());
                            }
                        } else if line_is_lastest {
                            if !line.trim().is_empty() {
                                let last_line_length = rowan::TextSize::new(line.trim_start().len() as u32);

                                string_last_line_range = Some(rowan::TextRange::at(
                                    content.text_range().end().checked_sub(last_line_length).unwrap(),
                                    last_line_length,
                                ));
                            } else if !part_is_multiline
                                && let Some(range) = previous_part_is_not_delimiter_range {
                                string_last_line_range = Some(range);
                            }
                        }

                        #[allow(clippy::nonminimal_bool)] if
                            // Ignore firstest and lastest lines.
                            !(line_is_firstest || line_is_lastest)
                            // Ignore lines right after an interpolation end.
                            && !(previous_part_is_not_delimiter_range.is_some() && line_is_first)
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
                                        self.text_range().into(),
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
                previous_part_is_not_delimiter_range = Some(part.text_range());
            }

            if part_is_multiline {
                string_is_multiline = true;
            }
        }

        if string_is_multiline {
            for range in [string_first_line_range, string_last_line_range].into_iter().flatten() {
                report.push_primary(
                    range.into(),
                    "multiline strings' first and last lines must be empty",
                );
            }
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

interpolated! {
    impl SString {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// RUNE

node! {
    #[from(NODE_RUNE)] struct Rune;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid rune");
        let mut reported_control_character = false;
        let mut reported_interpolation = false;

        let mut got_content = false;

        for part in self.parts() {
            match part {
                InterpolatedPart::Content(content) => {
                    let text = content.text();

                    match content.validate_escapes(&mut report) {
                        0 if text.chars().count() == 1 => {},
                        1 => {},

                        _ => {
                            report.push_primary(
                                content.text_range().into(),
                                "invalid rune literal length",
                            );
                        },
                    }

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(
                            content.text_range().into(),
                            "runes cannot contain control characters (non-escaped newlines, tabs, ...)",
                        );
                    }
                },

                InterpolatedPart::Interpolation(interpolation) if !reported_interpolation => {
                    reported_interpolation = true;
                    report.push_primary(
                        interpolation.text_range().into(),
                        "runes cannot contain interpolation",
                    );
                }

                _ => continue,
            }

            got_content = true;
        }

        if !got_content {
            report.push_primary(
                self.text_range().into(),
                "runes cannot be empty",
            );
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

interpolated! {
    impl Rune {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// ISLAND

node! {
    #[from(NODE_ISLAND)] struct Island;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut reported_control_character = false;

        for part in self.parts() {
            match part {
                InterpolatedPart::Content(content) => {
                    // content.validate_escapes(to);

                    let text = content.text();

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        to.push(Report::error(
                            "islands cannot contain control characters (non-escaped newlines, tabs, ...)").primary(
                            self.text_range().into(), "here"
                        ));
                    }
                },

                InterpolatedPart::Interpolation(interpolation) => {
                    interpolation.validate(to);
                },

                _ => {},
            }
        }
    }
}

interpolated! {
    impl Island {
        let content_kind = TOKEN_CONTENT;
        type ContentToken = token::Content;
    }
}

// NUMBER

node! { #[from(NODE_NUMBER)] struct Number; }

impl Number {
    /// Returns the underlying value of this number.
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

/// A Number value. May either be a [`token::Integer`] or a [`token::Float`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumberValue {
    Integer(token::Integer),
    Float(token::Float),
}

// IF THEN

node! {
    #[from(NODE_IF_THEN)] struct IfThen;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.condition().validate(to);
        self.consequence().validate(to);

        if let Some(alternative) = self.alternative() {
            alternative.validate(to);
        }
    }
}

impl IfThen {
    get_token! { if_token -> TOKEN_LITERAL_IF }

    get_node! { condition -> 0 @ Expression }

    get_token! { then_token -> TOKEN_LITERAL_THEN }

    get_node! { consequence -> 1 @ Expression }

    get_token! { else_token -> ? TOKEN_LITERAL_ELSE }

    get_node! { alternative -> 2 @ ? Expression }
}

// IF IS

node! {
    #[from(NODE_IF_IS)] struct IfIs;

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid if-is");

        self.expression().validate(to);

        for item in self.patterns().same_items() {
            match item {
                Expression::InfixOperation(operation) if let InfixOperator::Lambda = operation.operator() => {
                    operation.validate(to);
                },

                invalid => {
                    report.push_primary(
                        invalid.text_range().into(),
                        "invalid branch",
                    );
                    report.push_help("all if-is branches must be lambdas");
                },
            }
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

impl IfIs {
    get_token! { if_token -> TOKEN_LITERAL_IF }

    get_node! { expression -> 0 @ Expression }

    get_token! { is_token -> TOKEN_LITERAL_IS }

    get_node! { patterns -> 1 @ Expression }
}

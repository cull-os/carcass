//! [`Node`] definitions.
use std::{
    collections::VecDeque,
    ops,
};

use cab_report::Report;
use cab_text::{
    Range,
    Rangeable,
};
use enumset::{
    EnumSet,
    enum_set,
};
use smallvec::SmallVec;

use crate::{
    FromRed,
    Kind::{
        self,
        *,
    },
    red,
    token,
};

macro_rules! node {
    (
        #[from($kind:ident)]
        struct $name:ident;
    ) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) red::Node);

        impl ops::Deref for $name {
            type Target = red::Node;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl FromRed<red::Node> for $name {
            const KINDS: EnumSet<Kind> = enum_set!($kind);

            fn can_cast(node: &red::Node) -> bool {
                Self::KINDS.contains(node.kind())
            }

            fn cast(node: red::Node) -> Option<Self> {
                Self::can_cast(node.kind()).then_some(Self(node))
            }
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

        impl ops::Deref for $name {
            type Target = red::Node;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl FromRed<red::Node> for $name {
            const KINDS: EnumSet<Kind> = enum_set!($($variant::KIND)|*);

            fn can_cast(node: &red::Node) -> bool {
                Self::KINDS.contains(node.kind())
            }

            fn cast(node: red::Node) -> Option<Self> {
                match node.kind() {
                    $($variant::KIND => Some(Self::$variant($variant(node))),)*
                    _ => None,
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

macro_rules! token_get {
    ($name:ident -> $($skip:literal @)? Option<$kind:ident>) => {
        pub fn $name(&self) -> Option<&red::Token> {
            self.children_with_tokens()
                .filter_map(red::Element::into_token)
                $(.skip($skip))?
                .find(|token| token.kind() == $kind)
        }
    };

    ($name:ident -> $($skip:literal @)? $kind:ident) => {
        pub fn $name(&self) -> &red::Token {
            self.children_with_tokens()
                .filter_map(red::Element::into_token)
                $(.skip($skip))?
                .find(|token| token.kind() == $kind)
                .expect("node must have a token child")
        }
    };
}

macro_rules! node__get {
    ($name:ident -> $($skip:literal @)? Option<$type:ty>) => {
        pub fn $name(&self) -> Option<$type> {
            self.children()
                .filter_map(|node| <$type>::can_cast(node.kind()))
                $(.skip($skip))?
                .next()
                .map(|node| <$type>::cast(node.clone()).unwrap())
        }
    };

    ($name:ident -> $($skip:literal @)? $type:ty) => {
        pub fn $name(&self) -> $type {
            self.children()
                .filter(|node| <$type>::can_cast(node.kind()))
                $(.skip($skip))?
                .next()
                .map(|node| <$type>::cast(node.clone()).unwrap())
                .expect("node must have a matching node child")
        }
    };

    ($name:ident -> impl Iterator<Item = $type:ty>) => {
        pub fn $name(&self) -> impl Iterator<Item = &<$type>> {
            self.children()
                .filter(|node| <$type>::can_cast(node.kind()))
                .map(|node| <$type>::cast(node.clone()).unwrap())
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
}

impl Expression {
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
                                .primary(invalid.range(), "left operand must be an identifier"),
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
                    report.push_primary(bind_range, "bind");
                }

                if report.labels.len() > 1 {
                    to.push(report);
                }
            },
        }
    }

    fn validate_pattern_arithmetic(&self, bind_ranges: &mut SmallVec<Range, 4>, reports: &mut Vec<Report<'_>>) {
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
                        .primary(self.range(), "here"),
                );
            },

            Expression::Identifier(identifier) => {
                identifier.validate(reports);
                bind_ranges.push(identifier.range());
            },

            Expression::SString(string) => {
                string.validate(reports);
            },

            Expression::Number(number) => {
                number.validate(reports);
            },

            _ => {
                reports.push(Report::error("invalid pattern").primary(
                    self.range(),
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

node! { #[from(NODE_PARENTHESIS)] struct Parenthesis; }

impl Parenthesis {
    token_get! { token_left_parenthesis -> TOKEN_LEFT_PARENTHESIS }

    node__get! { expression -> Expression }

    token_get! { token_right_parenthesis -> Option<TOKEN_RIGHT_PARENTHESIS> }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.expression().validate(to);
    }
}

// LIST

node! { #[from(NODE_LIST)] struct List; }

impl List {
    token_get! { token_left_bracket -> TOKEN_LEFT_BRACKET }

    node__get! { expression -> Option<Expression> }

    token_get! { token_right_bracket -> Option<TOKEN_RIGHT_BRACKET> }

    /// Returns all items of the list.
    pub fn items(&self) -> impl Iterator<Item = Expression> {
        self.expression().into_iter().flat_map(Expression::same_items)
    }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        if let Some(Expression::InfixOperation(operation)) = self.expression()
            && operation.operator() == InfixOperator::Sequence
        {
            to.push(
                Report::error("inner expression of list cannot be sequence")
                    .primary(operation.range(), "consider parenthesizing this"),
            );
        }

        for item in self.items() {
            item.validate(to);
        }
    }
}

// ATTRIBUTE LIST

node! { #[from(NODE_ATTRIBUTE_LIST)] struct AttributeList; }

impl AttributeList {
    token_get! { left_curlybrace_token -> TOKEN_LEFT_CURLYBRACE }

    node__get! { expression -> Option<Expression> }

    token_get! { right_curlybrace_token -> Option<TOKEN_RIGHT_CURLYBRACE> }

    /// Returns all entries of the attribute list.
    pub fn entries(&self) -> impl Iterator<Item = Expression> {
        self.expression().into_iter().flat_map(Expression::same_items)
    }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        for entry in self.entries() {
            match entry {
                Expression::InfixOperation(operation) if let InfixOperator::Sequence = operation.operator() => {
                    to.push(
                        Report::error("unexpected sequence operator inside attribute list").primary(
                            operation.range(),
                            "sequence operator has lower binding power and will consume everything",
                        ),
                    );
                },

                Expression::InfixOperation(operation) if let InfixOperator::Bind = operation.operator() => {
                    operation.validate(to);
                },

                Expression::Identifier(identifier) => {
                    identifier.validate(to);
                },

                invalid => to.push(Report::error("invalid attribute").primary(invalid.range(), "here")),
            }
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

node! { #[from(NODE_PREFIX_OPERATION)] struct PrefixOperation; }

impl PrefixOperation {
    node__get! { right -> 0 @ Expression }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> &red::Token {
        self.children_with_tokens()
            .filter_map(|child| child.into_token())
            .find(|token| PrefixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> PrefixOperator {
        self.children_red_tokens()
            .filter_map(|child| child.into_token())
            .find_map(|token| PrefixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
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

node! { #[from(NODE_INFIX_OPERATION)] struct InfixOperation; }

impl InfixOperation {
    node__get! { left -> 0 @ Expression }

    node__get! { right -> 1 @ Expression }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> Option<&red::Token> {
        self.children_with_tokens()
            .filter_map(|child| child.into_token())
            .find(|token| InfixOperator::try_from(token.kind()).is_ok())
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> InfixOperator {
        self.children_with_tokens()
            .filter_map(|child| child.into_token())
            .find_map(|token| InfixOperator::try_from(token.kind()).ok())
            .unwrap_or(InfixOperator::ImplicitApply)
    }

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
                                    .primary(item.range(), "operand")
                                    .help(
                                        "all same-operands must be of matching type: either all lambdas or all binds",
                                    ),
                            );
                            continue;
                        }

                        operation.validate(to);
                    } else {
                        to.push(
                            Report::error("invalid same-operand")
                                .primary(item.range(), "operand")
                                .help("all same-operands must either be lambdas or binds"),
                        );
                    }
                }
            },

            InfixOperator::This => {
                self.validate_left(to);
                self.right().validate(to);
            },

            InfixOperator::Lambda | InfixOperator::Bind => {
                self.left().validate_pattern(to);
                self.right().validate(to);
            },

            operator => {
                let expressions = &[self.left(), self.right()];

                for expression in expressions {
                    expression.validate(to);
                }

                let (InfixOperator::Apply | InfixOperator::Pipe) = operator else {
                    return;
                };

                for expression in expressions {
                    if let Expression::InfixOperation(operation) = expression
                        && let child_operator @ (InfixOperator::Apply | InfixOperator::Pipe) = operation.operator()
                        && child_operator != operator
                    {
                        to.push(
                            Report::error("application and pipe operators do not associate")
                                .secondary(self.range(), "this")
                                .primary(operation.range(), "does not associate with this"),
                        );
                    }
                }
            },
        }
    }

    /// Asserts that this node is a this-expression and validates the left
    /// expression.
    fn validate_left(&self, to: &mut Vec<Report<'_>>) {
        assert_eq!(self.operator(), InfixOperator::This);

        match self.left() {
            Expression::Identifier(identifier) => {
                identifier.validate(to);
            },

            invalid => {
                to.push(
                    Report::error("left operand of a this-expression must be an identifier")
                        .primary(invalid.range(), "left operand"),
                );
            },
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

    fn try_from(from: Kind) -> Result<Self, Self::Error> {
        match from {
            TOKEN_COMMA => Ok(Self::Same),
            TOKEN_SEMICOLON => Ok(Self::Sequence),

            _ => Err(()),
        }
    }
}

node! { #[from(NODE_SUFFIX_OPERATION)] struct SuffixOperation; }

impl SuffixOperation {
    node__get! { left -> 0 @ Expression }

    /// Returns the operator token of this operation.
    pub fn operator_token(&self) -> &red::Token {
        self.children_with_tokens()
            .filter_map(|child| child.into_token())
            .find(|token| SuffixOperator::try_from(token.kind()).is_ok())
            .unwrap()
    }

    /// Returns the operator of this operation.
    pub fn operator(&self) -> SuffixOperator {
        self.children_with_tokens()
            .filter_map(|child| child.into_token())
            .find_map(|token| SuffixOperator::try_from(token.kind()).ok())
            .unwrap()
    }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.left().validate(to);
    }
}

// INTERPOLATION

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum InterpolatedPart<Token: FromRed<red::Token>> {
    Delimiter(red::Token),
    Content(Token),
    Interpolation(Interpolation),
}

impl<Token: FromRed<red::Token>> InterpolatedPart<Token> {
    pub fn is_delimiter(&self) -> bool {
        matches!(self, Self::Delimiter(_))
    }

    pub fn range(&self) -> Range {
        match self {
            InterpolatedPart::Delimiter(delimiter) => delimiter.range(),
            InterpolatedPart::Content(content) => content.range(),
            InterpolatedPart::Interpolation(interpolation) => interpolation.range(),
        }
    }
}

node! { #[from(NODE_INTERPOLATION)] struct Interpolation; }

impl Interpolation {
    token_get! { interpolation_start_token -> TOKEN_INTERPOLATION_START }

    node__get! { expression -> 0 @ Expression }

    token_get! { interpolation_end_token -> Option<TOKEN_INTERPOLATION_END> }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.expression().validate(to);
    }
}

macro_rules! parted {
    ($content_type:ty) => {
        pub fn parts(&self) -> impl Iterator<Item = InterpolatedPart<$content_type>> {
            self.children_with_tokens().map(|child| {
                match child {
                    red::NodeOrToken::Token(token) => {
                        if <$content_type>::can_cast(token) {
                            InterpolatedPart::Content(<$content_type>::cast(token.clone()).unwrap())
                        } else {
                            InterpolatedPart::Delimiter(token.clone())
                        }
                    },

                    red::NodeOrToken::Node(node) => {
                        InterpolatedPart::Interpolation(
                            Interpolation::cast(node.clone())
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

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        for part in self.parts() {
            if let InterpolatedPart::Interpolation(interpolation) = part {
                interpolation.validate(to);
            }
        }
    }
}

// IDENTIFIER

node! { #[from(NODE_IDENTIFIER)] struct IdentifierQuoted; }

impl IdentifierQuoted {
    parted!(token::Content);

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
                        report.push_primary(content.range(), "contains control characters");
                        report.push_help(
                            "quoted identifiers cannot contain control characters (non-escaped newlines, tabs, ...)",
                        );
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

/// An identifier value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum IdentifierValue {
    /// A plain identifier backed by a [`token::Identifier`].
    Plain(token::Identifier),
    /// A quoted identifier backed by a stringlike [`IdentifierQuoted`].
    Quoted(IdentifierQuoted),
}

node! { #[from(NODE_IDENTIFIER)] struct Identifier; }

impl Identifier {
    /// Returns the value of this identifier. A value may either be a
    /// [`token::Identifier`] or a quoted stringlike.
    pub fn value(&self) -> IdentifierValue {
        if let Some(token) = self
            .first_token()
            .and_then(|token| token::Identifier::cast(token.clone()))
        {
            return IdentifierValue::Plain(token);
        }

        if self
            .first_token()
            .is_some_and(|token| IdentifierQuoted::can_cast(token))
        {
            return IdentifierValue::Quoted(IdentifierQuoted((*self).clone()));
        }

        unreachable!("identifier node did not have an identifier or identifier starter token")
    }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        if let IdentifierValue::Quoted(quoted) = self.value() {
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

        let mut previous_interpolation_range = None;
        while let Some((part_index, part)) = parts.next() {
            let mut part_is_multiline = true;
            let part_is_first = part_index == 0;
            let part_is_last = parts.peek().is_none_or(|(_, part)| part.is_delimiter());

            match &part {
                InterpolatedPart::Interpolation(interpolation) => {
                    interpolation.validate(to);

                    let range = interpolation.range();

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
                                string_first_line_range = Some(Range::at(content.range().start, line.trim_end().len()));
                            } else if text.trim().is_empty()
                                && let Some((_, part)) = parts.peek()
                                && !part.is_delimiter()
                            {
                                string_first_line_range = Some(part.range());
                            }
                        } else if line_is_lastest {
                            if !line.trim().is_empty() {
                                let last_line_length = line.trim_start().len();

                                string_last_line_range = Some(Range::at_end(content.range().end, last_line_length));
                            } else if !part_is_multiline && let Some(range) = previous_interpolation_range {
                                string_last_line_range = Some(range);
                            }
                        }

                        #[allow(clippy::nonminimal_bool)]
                        if
                        // Ignore firstest and lastest lines.
                        !(line_is_firstest || line_is_lastest)
                            // Ignore lines right after an interpolation end.
                            && !(previous_interpolation_range.is_some() && line_is_first)
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
                                        self.range(),
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
                previous_interpolation_range = Some(part.range());
            }

            if part_is_multiline {
                string_is_multiline = true;
            }
        }

        if string_is_multiline {
            for range in [string_first_line_range, string_last_line_range].into_iter().flatten() {
                report.push_primary(range.into(), "multiline strings' first and last lines must be empty");
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

                        // <= 2 because all escapes are at most two characters wide.
                        1 => if text.as_bytes().len() <= 2 {},

                        _ => {
                            report.push_primary(content.range(), "invalid rune literal length");
                        },
                    }

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(
                            content.range(),
                            "runes cannot contain control characters (non-escaped newlines, tabs, ...)",
                        );
                    }
                },

                InterpolatedPart::Interpolation(interpolation) if !reported_interpolation => {
                    reported_interpolation = true;
                    report.push_primary(interpolation.range(), "runes cannot contain interpolation");
                },

                _ => continue,
            }

            got_content = true;
        }

        if !got_content {
            report.push_primary(self.range(), "runes cannot be empty");
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

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid island");
        let mut reported_control_character = false;

        for part in self.parts() {
            match part {
                InterpolatedPart::Content(content) => {
                    content.validate_escapes(&mut report);

                    let text = content.text();

                    if !reported_control_character && text.chars().any(char::is_control) {
                        reported_control_character = true;
                        report.push_primary(content.range(), "here");
                        report.push_tip("islands cannot contain control characters (non-escaped newlines, tabs, ...)");
                    }
                },

                InterpolatedPart::Interpolation(interpolation) => {
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

// NUMBER

/// A Number value. May either be a [`token::Integer`] or a [`token::Float`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NumberValue {
    Integer(token::Integer),
    Float(token::Float),
}

node! { #[from(NODE_NUMBER)] struct Number; }

impl Number {
    /// Returns the underlying value of this number.
    pub fn value(&self) -> NumberValue {
        if let Some(token) = self.first_token().and(|token| token::Integer::cast(token.clone())) {
            return NumberValue::Integer(token);
        }

        if let Some(token) = self.first_token().and(|token| token::Float::cast(token.clone())) {
            return NumberValue::Float(token);
        }

        unreachable!()
    }
}

// IF THEN

node! { #[from(NODE_IF_THEN)] struct IfThen; }

impl IfThen {
    token_get! { if_token -> TOKEN_LITERAL_IF }

    node__get! { condition -> 0 @ Expression }

    token_get! { then_token -> TOKEN_LITERAL_THEN }

    node__get! { consequence -> 1 @ Expression }

    token_get! { else_token -> Option<TOKEN_LITERAL_ELSE> }

    node__get! { alternative -> 2 @ Option<Expression> }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        self.condition().validate(to);
        self.consequence().validate(to);

        if let Some(alternative) = self.alternative() {
            alternative.validate(to);
        }
    }
}

// IF IS

node! { #[from(NODE_IF_IS)] struct IfIs; }

impl IfIs {
    token_get! { if_token -> TOKEN_LITERAL_IF }

    node__get! { expression -> 0 @ Expression }

    token_get! { is_token -> TOKEN_LITERAL_IS }

    node__get! { patterns -> 1 @ Expression }

    fn validate(&self, to: &mut Vec<Report<'_>>) {
        let mut report = Report::error("invalid if-is");

        self.expression().validate(to);

        for item in self.patterns().same_items() {
            match item {
                Expression::InfixOperation(operation) if let InfixOperator::Lambda = operation.operator() => {
                    operation.validate(to);
                },

                invalid => {
                    report.push_primary(invalid.range(), "invalid branch");
                    report.push_help("all if-is branches must be lambdas");
                },
            }
        }

        if !report.is_empty() {
            to.push(report);
        }
    }
}

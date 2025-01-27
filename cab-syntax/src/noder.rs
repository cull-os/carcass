use std::{
    borrow,
    fmt::Write as _,
    result,
};

use enumset::EnumSet;
use peekmore::{
    PeekMore as _,
    PeekMoreIterator as PeekMore,
};

use crate::{
    Kind::{
        self,
        *,
    },
    RowanNode,
    node,
};

/// A parse result that contains a [`rowan::SyntaxNode`],
/// contains a [`node::Node`] if it was successfully created,
/// and a list of [`NodeError`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse<N: node::Node> {
    /// The underlying [`rowan::SyntaxNode`].
    pub syntax: RowanNode,

    /// The [`node::Node`], if it was successfully created.
    pub node: Option<N>,

    /// Errors encountered during parsing.
    pub errors: Vec<NodeError>,
}

impl<N: node::Node> Parse<N> {
    /// Returns [`Ok`] with the [`node::Node`] node if there are no errors,
    /// returns [`Err`] with the list of errors obtained from
    /// [`Self::errors`] otherwise.
    pub fn result(self) -> result::Result<N, Vec<NodeError>> {
        if self.errors.is_empty() {
            Ok(self.node.unwrap())
        } else {
            Err(self.errors)
        }
    }
}

/// Options related to parsing.
#[derive(Debug, Clone)]
pub struct ParseOptions {
    /// Whether to deduplicate errors. For every error with the same range, the
    /// first one will be kept.
    ///
    /// This means that the following string will only produce a single error,
    /// instead of two:
    ///
    /// ```txt
    /// [{ a := (b]
    ///           ^ expected ')', found ']'
    ///           ^ expected '}', found ']' (omitted if option is set)
    /// ```
    pub deduplicate_errors: bool,
}

impl Default for ParseOptions {
    fn default() -> Self {
        ParseOptions {
            deduplicate_errors: true,
        }
    }
}

/// Parses a token iterator and returns a [`Parse`].
///
/// Parsing will always fail if the given [`node::Node`] type is not an
/// expression as we cannot parse sub expressions.
pub fn parse<'a, I: Iterator<Item = (Kind, &'a str)>, N: node::Node>(
    tokens: I,
    options: ParseOptions,
) -> Parse<N> {
    let mut noder = Noder::new(tokens);

    noder.node(NODE_ROOT, |this| {
        this.node_expression(EnumSet::empty());
        this.next_expect(EnumSet::empty(), EnumSet::empty());
    });

    let syntax = RowanNode::new_root(noder.builder.finish());
    let node = syntax.first_child().and_then(|node| N::cast(node));
    let mut errors = noder.errors;

    // Handle unexpected node type. Happens when you
    // `parse::<node::Foo>(...)` and the input
    // contains a non-Foo expression.
    if node.is_none() {
        let node = syntax.first_child();

        errors.push(NodeError::unexpected(
            node.as_ref().map(|node| node.kind()),
            N::kind(),
            node.as_ref()
                .map(|node| node.text_range())
                .unwrap_or_else(|| rowan::TextRange::empty(0.into())),
        ));
    }

    if options.deduplicate_errors {
        let mut last_text_range = None;

        errors.retain(move |NodeError { at, .. }| {
            if last_text_range != Some(at.start()) {
                last_text_range = Some(at.start());
                true
            } else {
                false
            }
        })
    }

    // Purposefully after error deduplication.
    if let Some(node) = &node {
        node.validate(&mut errors);
    }

    Parse {
        syntax,
        node,
        errors,
    }
}

/// A node error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeError {
    /// The error's reason.
    pub reason: borrow::Cow<'static, str>,
    /// Where the error happened in the source.
    pub at: rowan::TextRange,
}

impl NodeError {
    pub fn new(reason: impl Into<borrow::Cow<'static, str>>, at: rowan::TextRange) -> Self {
        Self {
            reason: reason.into(),
            at,
        }
    }

    pub fn invalid_pattern(got: Kind, at: rowan::TextRange) -> Self {
        Self {
            reason: format!("{got} is not a valid pattern").into(),
            at,
        }
    }

    pub fn unexpected(
        got: Option<Kind>,
        mut expected: EnumSet<Kind>,
        at: rowan::TextRange,
    ) -> NodeError {
        let mut reason = String::from("expected ");

        if expected == EnumSet::empty() {
            write!(reason, "end of file, got {got}", got = got.unwrap()).ok();

            return Self {
                reason: reason.into(),
                at,
            };
        }

        if expected.is_superset(Kind::EXPRESSION_SET) {
            expected.remove_all(Kind::EXPRESSION_SET);

            let separator = match expected.len() {
                0 => "",
                1 => " or ",
                2.. => ", ",
            };

            write!(reason, "an expression{separator}").ok();
        }

        if expected.is_superset(Kind::IDENTIFIER_SET) {
            expected.remove(TOKEN_IDENTIFIER_START);
        }

        for (index, item) in expected.into_iter().enumerate() {
            let position = index + 1;

            let separator = match position {
                position if expected.len() == position => "",
                position if expected.len() == position + 1 => " or ",
                _ => ", ",
            };

            write!(reason, "{item}{separator}").ok();
        }

        if let Some(got) = got {
            write!(reason, ", got {got}").ok();
        } else {
            write!(reason, ", reached end of file").ok();
        }

        Self {
            reason: reason.into(),
            at,
        }
    }
}

type Result<T> = result::Result<T, ()>;

struct Noder<'a, I: Iterator<Item = (Kind, &'a str)>> {
    builder: rowan::GreenNodeBuilder<'a>,

    tokens: PeekMore<I>,
    errors: Vec<NodeError>,

    offset: rowan::TextSize,
}

impl<'a, I: Iterator<Item = (Kind, &'a str)>> Noder<'a, I> {
    fn new(tokens: I) -> Self {
        Self {
            builder: rowan::GreenNodeBuilder::new(),

            tokens: tokens.peekmore(),
            errors: Vec::new(),

            offset: 0.into(),
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    fn node<T>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> T) -> T {
        self.builder.start_node(kind.into());

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn node_from<T>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.builder.start_node_at(checkpoint, kind.into());

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn peek_direct(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_nth(&mut self, n: usize) -> Option<Kind> {
        let mut peek_index: usize = 0;
        let mut index: usize = 0;

        loop {
            let &(kind, _) = self.tokens.peek_nth(peek_index)?;

            if index >= n && !kind.is_trivia() {
                return Some(kind);
            }

            peek_index += 1;

            if !kind.is_trivia() {
                index += 1;
            }
        }
    }

    fn peek(&mut self) -> Option<Kind> {
        self.peek_nth(0)
    }

    fn next_direct(&mut self) -> Result<Kind> {
        match self.tokens.next() {
            Some((kind, slice)) => {
                self.offset += rowan::TextSize::of(slice);
                self.builder.token(kind.into(), slice);

                Ok(kind)
            },

            None => {
                self.errors.push(NodeError::unexpected(
                    None,
                    EnumSet::empty(),
                    rowan::TextRange::empty(self.offset),
                ));

                Err(())
            },
        }
    }

    fn next_direct_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) {
        while self.peek_direct().is_some_and(&mut predicate) {
            self.next_direct().unwrap();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_direct_while(Kind::is_trivia)
    }

    fn next(&mut self) -> Result<Kind> {
        self.next_while_trivia();
        self.next_direct()
    }

    fn next_if(&mut self, expected: Kind) -> bool {
        let condition = self.peek() == Some(expected);

        if condition {
            self.next().unwrap();
        }

        condition
    }

    fn next_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) -> rowan::TextRange {
        let start = self.offset;

        while self.peek().is_some_and(&mut predicate) {
            self.next().unwrap();
        }

        rowan::TextRange::new(start, self.offset)
    }

    fn next_expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> Option<Kind> {
        let expected_at = self.checkpoint();

        match self.peek() {
            None if expected.is_empty() => None,
            Some(next) if expected.contains(next) => Some(self.next().unwrap()),

            unexpected => {
                let unexpected_range = self.next_while(|next| !(until | expected).contains(next));

                self.node_from(expected_at, NODE_ERROR, |_| {});

                self.errors.push(NodeError::unexpected(
                    unexpected,
                    expected,
                    unexpected_range,
                ));

                let next = self.peek()?;

                if expected.contains(next) {
                    Some(self.next().unwrap())
                } else {
                    None
                }
            },
        }
    }

    fn node_parenthesis(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_PARENTHESIS, |this| {
            this.next_expect(
                TOKEN_LEFT_PARENTHESIS.into(),
                until | Kind::EXPRESSION_SET | TOKEN_RIGHT_PARENTHESIS,
            );

            this.node_expression(until | TOKEN_RIGHT_PARENTHESIS);

            this.next_expect(TOKEN_RIGHT_PARENTHESIS.into(), until);
        });
    }

    fn node_list(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_LIST, |this| {
            this.next_expect(
                TOKEN_LEFT_BRACKET.into(),
                until | Kind::EXPRESSION_SET | TOKEN_RIGHT_BRACKET,
            );

            if this.peek() != Some(TOKEN_RIGHT_BRACKET) {
                this.node_expression(until | TOKEN_RIGHT_BRACKET);
            }

            this.next_expect(TOKEN_RIGHT_BRACKET.into(), until);
        });
    }

    fn node_attribute_list(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_ATTRIBUTE_LIST, |this| {
            this.next_expect(
                TOKEN_LEFT_CURLYBRACE.into(),
                until | Kind::EXPRESSION_SET | TOKEN_RIGHT_CURLYBRACE,
            );

            if this.peek() != Some(TOKEN_RIGHT_CURLYBRACE) {
                this.node_expression(until | TOKEN_RIGHT_CURLYBRACE);
            }

            this.next_expect(TOKEN_RIGHT_CURLYBRACE.into(), until);
        });
    }

    fn node_path(&mut self) {
        self.node(NODE_PATH, |this| {
            loop {
                match this.peek_direct() {
                    Some(TOKEN_PATH) => {
                        this.next_direct().unwrap();
                    },

                    Some(TOKEN_INTERPOLATION_START) => {
                        this.node_interpolation();
                    },

                    _ => break,
                }
            }
        });
    }

    fn node_identifier(&mut self, until: EnumSet<Kind>) {
        if self.peek() == Some(TOKEN_IDENTIFIER_START) {
            self.node_stringlike();
        } else {
            self.node(NODE_IDENTIFIER, |this| {
                this.next_expect(Kind::IDENTIFIER_SET, until);
            });
        }
    }

    fn node_stringlike(&mut self) {
        let start_of_stringlike = self.checkpoint();

        let (node, end) = match self.next().ok() {
            Some(TOKEN_IDENTIFIER_START) => (NODE_IDENTIFIER, TOKEN_IDENTIFIER_END),
            Some(TOKEN_STRING_START) => (NODE_STRING, TOKEN_STRING_END),
            Some(TOKEN_ISLAND_START) => (NODE_ISLAND, TOKEN_ISLAND_END),
            _ => unreachable!(),
        };

        self.node_from(start_of_stringlike, node, |this| {
            loop {
                match this.peek() {
                    Some(TOKEN_CONTENT) => {
                        this.next_direct().unwrap();
                    },

                    Some(TOKEN_INTERPOLATION_START) => {
                        this.node_interpolation();
                    },

                    Some(other) if other == end => {
                        this.next_direct().unwrap();
                        break;
                    },

                    Some(_) => {
                        // Sometimes recoverably parsing interpolation leaves us unwanted tokens. It
                        // is not worth it trying to node it correctly without a big rewrite, so
                        // just consume them.
                        this.next_direct().unwrap();
                    },

                    None => {
                        this.errors.push(NodeError::unexpected(
                            None,
                            TOKEN_CONTENT | end,
                            rowan::TextRange::empty(this.offset),
                        ));
                        break;
                    },
                }
            }
        });

        // Treat expressions such as <foo>/bar as applciations. The evaluator will
        // special case on island<->path applications that contain literals to be path
        // accesses as islands are just virtual path roots anyway. Normally you cannot
        // use an island as a functor, I must say. That will be a hard error.
        if node == NODE_ISLAND && self.peek_direct() == Some(TOKEN_PATH) {
            self.node_from(start_of_stringlike, NODE_INFIX_OPERATION, |this| {
                this.node_path();
            });
        }
    }

    fn node_interpolation(&mut self) {
        self.node(NODE_INTERPOLATION, |this| {
            this.next_expect(TOKEN_INTERPOLATION_START.into(), EnumSet::empty());

            this.node_expression(TOKEN_INTERPOLATION_END.into());

            this.next_expect(TOKEN_INTERPOLATION_END.into(), EnumSet::empty());
        });
    }

    fn node_number(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_NUMBER, |this| {
            this.next_expect(TOKEN_INTEGER | TOKEN_FLOAT, until);
        });
    }

    fn node_if(&mut self, until: EnumSet<Kind>) {
        let then_else_binding_power = node::InfixOperator::Same.binding_power().0 + 1;
        let is_binding_power = node::InfixOperator::Sequence.binding_power().0 + 1;

        let start_of_if = self.checkpoint();

        self.next_expect(
            TOKEN_LITERAL_IF.into(),
            until
                | Kind::EXPRESSION_SET
                | TOKEN_LITERAL_IS
                | TOKEN_LITERAL_THEN
                | TOKEN_LITERAL_ELSE,
        );

        self.node_expression_binding_power(
            then_else_binding_power,
            until | TOKEN_LITERAL_IS | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
        );

        match self.next_expect(
            TOKEN_LITERAL_IS | TOKEN_LITERAL_THEN,
            until | TOKEN_LITERAL_ELSE,
        ) {
            Some(TOKEN_LITERAL_IS) => {
                self.node_from(start_of_if, NODE_IF_IS, |this| {
                    this.node_expression_binding_power(is_binding_power, until)
                })
            },

            Some(TOKEN_LITERAL_THEN) => {
                self.node_from(start_of_if, NODE_IF_ELSE, |this| {
                    this.node_expression_binding_power(
                        then_else_binding_power,
                        until | TOKEN_LITERAL_ELSE,
                    );

                    if this.next_if(TOKEN_LITERAL_ELSE) {
                        this.node_expression_binding_power(then_else_binding_power, until);
                    }
                });
            },

            None => {
                self.node_from(start_of_if, NODE_ERROR, |_| {});
            },

            _ => unreachable!(),
        }
    }

    fn node_expression_single(&mut self, until: EnumSet<Kind>) {
        let expected_at = self.checkpoint();

        match self.peek() {
            Some(TOKEN_LEFT_PARENTHESIS) => self.node_parenthesis(until),

            Some(TOKEN_LEFT_BRACKET) => self.node_list(until),

            Some(TOKEN_LEFT_CURLYBRACE) => self.node_attribute_list(until),

            Some(TOKEN_PATH) => self.node_path(),

            Some(next) if Kind::IDENTIFIER_SET.contains(next) => self.node_identifier(until),

            Some(TOKEN_STRING_START | TOKEN_ISLAND_START) => self.node_stringlike(),

            Some(TOKEN_INTEGER | TOKEN_FLOAT) => self.node_number(until),

            Some(TOKEN_LITERAL_IF) => self.node_if(until),

            unexpected => {
                // Consume until the next token is either the limit, an
                // expression token or an operator.
                let unexpected_range = self.next_while(|kind| {
                    !((until | Kind::EXPRESSION_SET).contains(kind)
                        || node::PrefixOperator::try_from(kind).is_ok()
                        || node::InfixOperator::try_from(kind)
                            .is_ok_and(|operator| operator.is_token_owning())
                        || node::SuffixOperator::try_from(kind).is_ok())
                });

                self.node_from(expected_at, NODE_ERROR, |_| {});

                self.errors.push(NodeError::unexpected(
                    unexpected,
                    Kind::EXPRESSION_SET,
                    unexpected_range,
                ));
            },
        }
    }

    fn node_expression_binding_power(&mut self, minimum_power: u16, until: EnumSet<Kind>) {
        let start_of_expression = self.checkpoint();

        if let Some(operator) = self
            .peek()
            .and_then(|kind| node::PrefixOperator::try_from(kind).ok())
        {
            let ((), right_power) = operator.binding_power();

            self.node(NODE_PREFIX_OPERATION, |this| {
                this.next().unwrap();
                this.node_expression_binding_power(right_power, until);
            });
        } else {
            self.node_expression_single(until);
        }

        while let Some(operator) = self
            .peek()
            .and_then(|kind| node::InfixOperator::try_from(kind).ok())
        {
            let (left_power, right_power) = operator.binding_power();
            if left_power < minimum_power {
                break;
            }

            let operator_token = operator.is_token_owning().then(|| self.next().unwrap());

            // Handle suffix-able infix operators. Not for purely suffix operators.
            if operator_token.is_some()
                && node::SuffixOperator::try_from(operator_token.unwrap()).is_ok()
                && self
                    .peek()
                    .is_none_or(|kind| !Kind::EXPRESSION_SET.contains(kind))
            {
                self.node_from(start_of_expression, NODE_SUFFIX_OPERATION, |_| {});
            } else {
                self.node_from(start_of_expression, NODE_INFIX_OPERATION, |this| {
                    this.node_expression_binding_power(right_power, until);
                });
            }
        }
    }

    fn node_expression(&mut self, until: EnumSet<Kind>) {
        self.node_expression_binding_power(0, until);
    }
}

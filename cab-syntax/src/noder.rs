use std::fmt;

use enumset::{
    EnumSet,
    enum_set,
};
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
    pub fn result(self) -> Result<N, Vec<NodeError>> {
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
        let start_of_input = this.checkpoint();

        // Reached an unrecoverable error.
        if let Err(error) = this.node_expression(EnumSet::empty()) {
            this.node_from(start_of_input, NODE_ERROR, |_| {});
            this.errors.push(error);
        }

        if let Some(unexpected) = this.peek() {
            let start = this.offset;

            this.node(NODE_ERROR, |this| this.next_direct_while(|_| true));
            this.errors.push(NodeError::Unexpected {
                got: Some(unexpected),
                expected: EnumSet::empty(),
                at: rowan::TextRange::new(start, this.offset),
            });
        }
    });

    let syntax = RowanNode::new_root(noder.builder.finish());
    let node = N::cast(syntax.first_child().unwrap());
    let mut errors = noder.errors;

    // Handle unexpected node type. Happens when you
    // `parse::<node::Foo>(...)` and the input
    // contains a non-Foo expression.
    if node.is_none()
        && let Some(expected) = N::inherent_kind()
    {
        let child = syntax.first_child().unwrap();

        errors.push(NodeError::Unexpected {
            got: Some(child.kind()),
            expected: expected.into(),
            at: child.text_range(),
        })
    }

    if options.deduplicate_errors {
        let mut last_text_range = None;

        errors.retain(move |error| {
            let NodeError::Unexpected { at, .. } = error else {
                return true;
            };

            if last_text_range != Some(*at) {
                last_text_range = Some(*at);
                true
            } else {
                false
            }
        })
    }

    // TODO: Temporary hack to check the string validation.
    if syntax.first_child().unwrap().first_token().unwrap().kind() == TOKEN_STRING_START {
        use rowan::ast::AstNode as _;

        let sstring = node::SString::cast(syntax.first_child().unwrap()).unwrap();

        let contents: Vec<_> = sstring
            .parts()
            .filter_map(|part| {
                if let node::InterpolationPart::Content(content) = part {
                    Some(content)
                } else {
                    None
                }
            })
            .collect();

        if let Err(errors2) = crate::token::Content::normalized(&contents) {
            errors.extend(errors2);
        }
    }

    Parse {
        syntax,
        node,
        errors,
    }
}

type MustNodeResult<T = ()> = Result<T, NodeError>;

type NodeResult<T = ()> = Result<Option<T>, NodeError>;

fn found<T>(value: T) -> NodeResult<T> {
    Ok(Some(value))
}

fn recoverable<T>() -> NodeResult<T> {
    Ok(None)
}

fn deadly<T>(error: NodeError) -> NodeResult<T> {
    Err(error)
}

const EXPRESSION_TOKENS: EnumSet<Kind> = enum_set!(
    TOKEN_LEFT_PARENTHESIS
        | TOKEN_LEFT_BRACKET
        | TOKEN_LEFT_CURLYBRACE
        | TOKEN_INTEGER
        | TOKEN_FLOAT
        | TOKEN_PATH
        | TOKEN_IDENTIFIER
        | TOKEN_IDENTIFIER_START
        | TOKEN_STRING_START
        | TOKEN_ISLAND_START
        | TOKEN_LITERAL_IF
);

const IDENTIFIER_TOKENS: EnumSet<Kind> = enum_set!(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START);

/// A node error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeError {
    /// An error that happens when the parsed expression is not a valid pattern.
    InvalidPattern {
        /// The node that was not a valid pattern.
        got: Option<Kind>,
        /// The range that contains the invalid node.
        at: rowan::TextRange,
    },

    // An error that happens when a stringlike contains invalid escapes or is formatted wrongly.
    InvalidStringlike {
        reason: &'static str,
        at: rowan::TextRange,
    },

    /// An error that happens when the noder was not expecting a particular
    /// token or node.
    Unexpected {
        /// The token that was not expected. This being [`None`] means that the
        /// end of the file was reached.
        got: Option<Kind>,
        /// The expected token. This being [`EnumSet::empty()`] means that the
        /// end of the file was expected, but there were leftovers.
        expected: EnumSet<Kind>,
        /// The range that contains the unexpected token sequence.
        at: rowan::TextRange,
    },
}

impl fmt::Display for NodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPattern { got, .. } => {
                if let Some(got) = got {
                    write!(formatter, "{got} is not a valid pattern")
                } else {
                    write!(formatter, "no pattern to be found")
                }
            },

            Self::InvalidStringlike { reason, .. } => write!(formatter, "{reason}"),

            Self::Unexpected {
                got: Some(got),
                expected: const { EnumSet::empty() },
                ..
            } => {
                write!(formatter, "expected end of file, got {got}")
            },

            &Self::Unexpected {
                got, mut expected, ..
            } => {
                write!(formatter, "expected ")?;

                if expected.is_superset(EXPRESSION_TOKENS) {
                    expected.remove_all(EXPRESSION_TOKENS);
                    write!(formatter, "an expression")?;

                    match expected.iter().count() {
                        0 => {},
                        1 => write!(formatter, " or ")?,
                        2.. => write!(formatter, ", ")?,
                    }
                }

                if expected.is_superset(IDENTIFIER_TOKENS) {
                    expected.remove(TOKEN_IDENTIFIER_START);
                }

                let mut iterator = expected.iter().peekmore();
                while let Some(item) = iterator.next() {
                    write!(
                        formatter,
                        "{item}{separator}",
                        separator =
                            match (iterator.peek().is_some(), iterator.peek_nth(1).is_some()) {
                                (false, false) => "",
                                (true, false) => " or ",
                                (true, true) => ", ",
                                _ => unreachable!(),
                            }
                    )?;
                }

                if let Some(got) = got {
                    write!(formatter, ", got {got}")
                } else {
                    write!(formatter, ", reached end of file")
                }
            },
        }
    }
}

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

    fn node_failable<K>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> NodeResult<K>) {
        let start_of_node = self.checkpoint();

        if let Err(error) = self.node(kind, closure) {
            self.node_from(start_of_node, NODE_ERROR, |_| {});
            self.errors.push(error);
        }
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

    fn node_failable_from<K>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> NodeResult<K>,
    ) {
        if let Err(error) = self.node_from(checkpoint, kind, closure) {
            self.node_from(checkpoint, NODE_ERROR, |_| {});
            self.errors.push(error);
        }
    }

    fn peek_direct(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_direct_expecting(&mut self, expected: EnumSet<Kind>) -> MustNodeResult<Kind> {
        self.peek_direct().ok_or(NodeError::Unexpected {
            got: None,
            expected,
            at: rowan::TextRange::empty(self.offset),
        })
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

    fn peek_expecting(&mut self, expected: EnumSet<Kind>) -> MustNodeResult<Kind> {
        self.peek().ok_or(NodeError::Unexpected {
            got: None,
            expected,
            at: rowan::TextRange::empty(self.offset),
        })
    }

    fn next_direct(&mut self) -> MustNodeResult<Kind> {
        self.tokens
            .next()
            .map(|(kind, slice)| {
                self.offset += rowan::TextSize::of(slice);
                self.builder.token(kind.into(), slice);
                kind
            })
            .ok_or(NodeError::Unexpected {
                got: None,
                expected: EnumSet::empty(),
                at: rowan::TextRange::empty(self.offset),
            })
    }

    fn next_direct_while(&mut self, predicate: impl Fn(Kind) -> bool) {
        while self.peek_direct().is_some_and(&predicate) {
            self.next_direct().unwrap();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_direct_while(Kind::is_trivia)
    }

    fn next(&mut self) -> MustNodeResult<Kind> {
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

    fn next_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) {
        while let Some(next) = self.peek() {
            if !predicate(next) {
                break;
            }

            self.next().unwrap();
        }
    }

    fn expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> NodeResult<Kind> {
        let expected_at = self.checkpoint();

        match self.peek_expecting(expected)? {
            next if expected.contains(next) => self.next().map(Some),

            unexpected => {
                let start = self.offset;

                self.node_from(expected_at, NODE_ERROR, |this| {
                    this.next_while(|kind| !(until | expected).contains(kind));
                });

                let error = NodeError::Unexpected {
                    got: Some(unexpected),
                    expected,
                    at: rowan::TextRange::new(start, self.offset),
                };

                let next = self.peek();

                if next.is_some_and(|kind| expected.contains(kind)) {
                    self.errors.push(error);
                    self.next().map(Some)
                } else if next.is_some() {
                    self.errors.push(error);
                    recoverable()
                } else {
                    deadly(error)
                }
            },
        }
    }

    fn node_parenthesis(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_PARENTHESIS, |this| {
            this.expect(
                TOKEN_LEFT_PARENTHESIS.into(),
                until | EXPRESSION_TOKENS | TOKEN_RIGHT_PARENTHESIS,
            )?;

            this.node_expression(until | TOKEN_RIGHT_PARENTHESIS)?;

            this.expect(TOKEN_RIGHT_PARENTHESIS.into(), until)
        });
    }

    fn node_list(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_LIST, |this| {
            this.expect(
                TOKEN_LEFT_BRACKET.into(),
                until | EXPRESSION_TOKENS | TOKEN_RIGHT_BRACKET,
            )?;

            if this.peek() != Some(TOKEN_RIGHT_BRACKET) {
                this.node_expression(until | TOKEN_RIGHT_BRACKET)?;
            }

            this.expect(TOKEN_RIGHT_BRACKET.into(), until)
        });
    }

    fn node_attribute_list(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_ATTRIBUTE_LIST, |this| {
            this.expect(
                TOKEN_LEFT_CURLYBRACE.into(),
                until | EXPRESSION_TOKENS | TOKEN_RIGHT_CURLYBRACE,
            )?;

            if this.peek() != Some(TOKEN_RIGHT_CURLYBRACE) {
                this.node_expression(until | TOKEN_RIGHT_CURLYBRACE)?;
            }

            this.expect(TOKEN_RIGHT_CURLYBRACE.into(), until)
        });
    }

    fn node_path(&mut self) {
        self.node_failable(NODE_PATH, |this| {
            loop {
                match this.peek_direct() {
                    Some(TOKEN_PATH) => {
                        this.next_direct().unwrap();
                    },

                    Some(TOKEN_INTERPOLATION_START) => {
                        this.node_interpolation()?;
                    },

                    _ => break found(()),
                }
            }
        });
    }

    fn node_identifier(&mut self, until: EnumSet<Kind>) {
        if self.peek() == Some(TOKEN_IDENTIFIER_START) {
            self.node_stringlike(TOKEN_IDENTIFIER_START, TOKEN_IDENTIFIER_END);
        } else {
            self.node_failable(NODE_IDENTIFIER, |this| {
                this.expect(IDENTIFIER_TOKENS, until)
            });
        }
    }

    fn node_stringlike(&mut self, start: Kind, end: Kind) {
        let node = match start {
            TOKEN_IDENTIFIER_START => NODE_IDENTIFIER,
            TOKEN_STRING_START => NODE_STRING,
            TOKEN_ISLAND_START => NODE_ISLAND,
            _ => unreachable!(),
        };

        self.node_failable(node, |this| {
            let current = this.next();
            assert_eq!(current, Ok(start));

            loop {
                match this.peek_direct_expecting(TOKEN_CONTENT | end)? {
                    TOKEN_CONTENT => {
                        this.next_direct().unwrap();
                    },

                    TOKEN_INTERPOLATION_START => {
                        this.node_interpolation()?;
                    },

                    other if other == end => {
                        this.next_direct().unwrap();
                        break found(());
                    },

                    _ => {
                        // Sometimes recoverably parsing interpolation leaves us unwanted tokens. It
                        // is not worth it trying to node it correctly without a big rewrite, so
                        // just consume them.
                        this.next_direct().unwrap();
                    },
                }
            }
        });
    }

    fn node_interpolation(&mut self) -> NodeResult {
        self.node(NODE_INTERPOLATION, |this| {
            this.expect(TOKEN_INTERPOLATION_START.into(), EnumSet::empty())?;

            this.node_expression(TOKEN_INTERPOLATION_END.into())?;

            this.expect(TOKEN_INTERPOLATION_END.into(), EnumSet::empty())?;

            found(())
        })
    }

    fn node_number(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_NUMBER, |this| {
            this.expect(TOKEN_INTEGER | TOKEN_FLOAT, until)
        });
    }

    fn node_if(&mut self, until: EnumSet<Kind>) -> NodeResult {
        let is_binding_power = node::InfixOperator::Sequence.binding_power().0 + 1;
        let then_else_binding_power = node::InfixOperator::Same.binding_power().0 + 1;

        let start_of_if = self.checkpoint();

        self.expect(
            TOKEN_LITERAL_IF.into(),
            until | EXPRESSION_TOKENS | TOKEN_LITERAL_IS | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
        )?;

        self.node_expression_binding_power(
            then_else_binding_power,
            until | TOKEN_LITERAL_IS | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
        )?;

        match self.expect(
            TOKEN_LITERAL_IS | TOKEN_LITERAL_THEN,
            until | TOKEN_LITERAL_ELSE,
        )? {
            Some(TOKEN_LITERAL_IS) => {
                self.node_from(start_of_if, NODE_IF_IS, |this| {
                    this.node_expression_binding_power(is_binding_power, until)
                })
            },

            Some(TOKEN_LITERAL_THEN) => {
                self.node_failable_from(start_of_if, NODE_IF_ELSE, |this| {
                    this.node_expression_binding_power(
                        then_else_binding_power,
                        until | TOKEN_LITERAL_ELSE,
                    )?;

                    if this.next_if(TOKEN_LITERAL_ELSE) {
                        this.node_expression_binding_power(then_else_binding_power, until)?;
                    }

                    found(())
                });

                found(())
            },

            _ => recoverable(),
        }
    }

    fn node_expression_single(&mut self, until: EnumSet<Kind>) -> NodeResult {
        match self.peek_expecting(EXPRESSION_TOKENS)? {
            TOKEN_LEFT_PARENTHESIS => self.node_parenthesis(until),

            TOKEN_LEFT_BRACKET => self.node_list(until),

            TOKEN_LEFT_CURLYBRACE => self.node_attribute_list(until),

            kind if IDENTIFIER_TOKENS.contains(kind) => self.node_identifier(until),

            TOKEN_PATH => self.node_path(),

            TOKEN_STRING_START => self.node_stringlike(TOKEN_STRING_START, TOKEN_STRING_END),

            TOKEN_ISLAND_START => self.node_stringlike(TOKEN_ISLAND_START, TOKEN_ISLAND_END),

            TOKEN_INTEGER | TOKEN_FLOAT => self.node_number(until),

            TOKEN_LITERAL_IF => {
                self.node_if(until)?;
            },

            unexpected => {
                self.next_while_trivia();
                let start = self.offset;

                self.node(NODE_ERROR, |this| {
                    // Consume until the next token is either the limit, an expression token or
                    // an operator.
                    this.next_while(|kind| {
                        !(until.contains(kind)
                            || EXPRESSION_TOKENS.contains(kind)
                            || node::PrefixOperator::try_from(kind).is_ok()
                            || node::InfixOperator::try_from(kind)
                                .is_ok_and(|operator| operator.is_token_owning())
                            || node::SuffixOperator::try_from(kind).is_ok())
                    });
                });

                self.errors.push(NodeError::Unexpected {
                    got: Some(unexpected),
                    expected: EXPRESSION_TOKENS,
                    at: rowan::TextRange::new(start, self.offset),
                });

                return recoverable();
            },
        }

        found(())
    }

    fn node_expression_binding_power(
        &mut self,
        minimum_power: u16,
        until: EnumSet<Kind>,
    ) -> NodeResult {
        let start_of_expression = self.checkpoint();

        if let Some(operator) = self
            .peek()
            .and_then(|kind| node::PrefixOperator::try_from(kind).ok())
        {
            let ((), right_power) = operator.binding_power();

            self.node_failable(NODE_PREFIX_OPERATION, |this| {
                this.next().unwrap();
                this.node_expression_binding_power(right_power, until)
            });
        } else {
            self.node_expression_single(until)?;
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
                    .is_none_or(|kind| !EXPRESSION_TOKENS.contains(kind))
            {
                self.node_from(start_of_expression, NODE_SUFFIX_OPERATION, |_| {});
            } else {
                self.node_failable_from(start_of_expression, NODE_INFIX_OPERATION, |this| {
                    this.node_expression_binding_power(right_power, until)
                });
            }
        }

        found(())
    }

    fn node_expression(&mut self, until: EnumSet<Kind>) -> NodeResult {
        self.node_expression_binding_power(0, until)
    }
}

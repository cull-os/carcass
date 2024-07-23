use std::{
    convert::Infallible,
    fmt,
    ops::{
        ControlFlow,
        FromResidual,
        Try,
    },
};

use enumset::{
    enum_set,
    EnumSet,
};
use peekmore::{
    PeekMore as _,
    PeekMoreIterator,
};
use rowan::{
    ast::AstNode as _,
    Language as _,
};

use crate::{
    node::{
        InfixOperator,
        Root,
    },
    tokenize,
    Kind::{
        self,
        *,
    },
    Language,
    RowanNode,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
enum ExpectMust<K = Kind> {
    Found(K),
    Deadly(Option<ParseError>),
}

impl<K> FromResidual for ExpectMust<K> {
    fn from_residual(residual: <Self as Try>::Residual) -> Self {
        match residual {
            Expect::Recoverable => Self::Deadly(None),
            Expect::Deadly(error) => Self::Deadly(Some(error)),
            _ => unreachable!(),
        }
    }
}

impl<K> Try for ExpectMust<K> {
    type Output = K;
    type Residual = Expect<Infallible>;

    fn from_output(output: Self::Output) -> Self {
        Self::Found(output)
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::Found(found) => ControlFlow::Continue(found),
            Self::Deadly(None) => ControlFlow::Break(Expect::Recoverable),
            Self::Deadly(Some(error)) => ControlFlow::Break(Expect::Deadly(error)),
        }
    }
}

impl<K> From<Result<K, ParseError>> for ExpectMust<K> {
    fn from(result: Result<K, ParseError>) -> Self {
        match result {
            Ok(value) => Self::Found(value),
            Err(error) => Self::Deadly(Some(error)),
        }
    }
}

impl<K> ExpectMust<K> {
    fn maybe(self) -> Expect<K> {
        match self {
            Self::Found(found) => Expect::Found(found),
            Self::Deadly(None) => Expect::Recoverable,
            Self::Deadly(Some(error)) => Expect::Deadly(error),
        }
    }

    fn unwrap(self) -> K {
        match self {
            Self::Found(found) => found,
            _ => panic!("asserted on a failed expect"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
enum Expect<K = Kind> {
    Found(K),
    Recoverable,
    Deadly(ParseError),
}

impl<K> FromResidual for Expect<K> {
    fn from_residual(residual: <Self as Try>::Residual) -> Self {
        match residual {
            Expect::Recoverable => Self::Recoverable,
            Expect::Deadly(error) => Self::Deadly(error),
            _ => unreachable!(),
        }
    }
}

impl<K> Try for Expect<K> {
    type Output = Option<K>;
    type Residual = Expect<Infallible>;

    fn from_output(output: Self::Output) -> Self {
        match output {
            Some(found) => Self::Found(found),
            None => Self::Recoverable,
        }
    }

    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::Found(found) => ControlFlow::Continue(Some(found)),
            Self::Recoverable => ControlFlow::Continue(None),
            Self::Deadly(error) => ControlFlow::Break(Expect::Deadly(error)),
        }
    }
}

impl<K> From<Result<K, ParseError>> for Expect<K> {
    fn from(result: Result<K, ParseError>) -> Self {
        match result {
            Ok(value) => Self::Found(value),
            Err(error) => Self::Deadly(error),
        }
    }
}

impl<K> Expect<K> {
    fn must(self) -> ExpectMust<K> {
        match self {
            Self::Found(found) => ExpectMust::Found(found),
            Self::Recoverable => ExpectMust::Deadly(None),
            Self::Deadly(error) => ExpectMust::Deadly(Some(error)),
        }
    }
}

const EXPRESSION_TOKENS: EnumSet<Kind> = enum_set!(
    TOKEN_LEFT_PARENTHESIS
        | TOKEN_LEFT_BRACKET
        | TOKEN_LEFT_CURLYBRACE
        | TOKEN_PLUS
        | TOKEN_MINUS
        | TOKEN_LITERAL_NOT
        | TOKEN_PATH
        | TOKEN_IDENTIFIER
        | TOKEN_IDENTIFIER_START
        | TOKEN_STRING_START
        | TOKEN_ISLAND_START
        | TOKEN_INTEGER
        | TOKEN_FLOAT
        | TOKEN_LITERAL_IF
);

const IDENTIFIER_TOKENS: EnumSet<Kind> = enum_set!(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START);

/// A parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// An error that happens when you nest expressions in too deep.
    ///
    /// No normal expression will ever hit this, but just in case.
    RecursionLimitExceeded {
        /// The starting point of the expression that was too nested.
        at: rowan::TextSize,
    },

    /// An error that happens when the parser was not expecting a particular
    /// token.
    Unexpected {
        /// The token that was not expected. This being [`None`] means that the
        /// end of the file was reached.
        got: Option<Kind>,
        /// The expected token. This being [`None`] means that the end of the
        /// file was expected, but there were leftovers.
        expected: EnumSet<Kind>,
        /// The range that contains the unexpected token sequence.
        at: rowan::TextRange,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecursionLimitExceeded { .. } => write!(formatter, "recursion limit exceeded"),

            Self::Unexpected {
                got: Some(got),
                expected: EnumSet::EMPTY,
                ..
            } => {
                write!(formatter, "expected end of file, got {got}")
            },

            Self::Unexpected {
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
                        "{item}{seperator}",
                        seperator =
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

/// A parse result that contains a [`rowan::GreenNode`] and a list of
/// [`ParseError`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    node: rowan::GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    /// Creates a [`RowanNode`] from the underlying GreenNode.
    pub fn syntax(self) -> RowanNode {
        RowanNode::new_root(self.node)
    }

    /// Returns an iterator over the underlying [`ParseError`]s, removing
    /// duplicates and only returning useful errors.
    pub fn errors(&self) -> Box<dyn Iterator<Item = &ParseError> + '_> {
        if self
            .errors
            .iter()
            .any(|error| matches!(error, ParseError::RecursionLimitExceeded { .. }))
        {
            return Box::new(
                self.errors
                    .iter()
                    .filter(|error| matches!(error, ParseError::RecursionLimitExceeded { .. })),
            );
        }

        let extra_error_count = self
            .errors
            .iter()
            .rev()
            .take_while(|error| matches!(error, ParseError::Unexpected { got: None, .. }))
            .map(|error| log::trace!("found end of file error: {error}"))
            .count() as isize
            - 1;

        Box::new(
            self.errors
                .iter()
                .take(self.errors.len() - extra_error_count.max(0) as usize),
        )
    }

    /// Creates a [`Root`] node from [`Self::syntax`].
    pub fn root(self) -> Root {
        Root::cast(self.syntax()).unwrap()
    }

    /// Returns [`Ok`] with the [`Root`] node if there are no errors, returns
    /// [`Err`] with the list of errors obtained from [`Self::errors`]
    /// otherwise.
    pub fn result(self) -> Result<Root, Vec<ParseError>> {
        if self.errors.is_empty() {
            Ok(self.root())
        } else {
            Err(self.errors().cloned().collect())
        }
    }
}

/// Parses a string reference and returns a [`Parse`].
pub fn parse(input: &str) -> Parse {
    let mut parser = Parser::new(tokenize(input));

    parser.node(NODE_ROOT, |this| {
        let checkpoint = this.checkpoint();

        // Reached an unrecoverable error.
        if let Expect::Deadly(error) = this.parse_expression(EnumSet::EMPTY) {
            log::trace!("unrecoverable error encountered: {error:?}");

            this.node_from(checkpoint, NODE_ERROR, |_| {});
            this.errors.push(error);
        }

        if let Some(got) = this.peek() {
            log::trace!("leftovers encountered: {got:?}");

            let start = this.offset;

            this.node(NODE_ERROR, |this| this.next_direct_while(|_| true));
            this.errors.push(ParseError::Unexpected {
                got: Some(got),
                expected: EnumSet::EMPTY,
                at: rowan::TextRange::new(start, this.offset),
            });
        }
    });

    Parse {
        node: parser.builder.finish(),
        errors: parser.errors,
    }
}

struct Parser<'a, I: Iterator<Item = (Kind, &'a str)>> {
    builder: rowan::GreenNodeBuilder<'a>,

    tokens: PeekMoreIterator<I>,
    errors: Vec<ParseError>,

    offset: rowan::TextSize,
    depth: u32,
}

impl<'a, I: Iterator<Item = (Kind, &'a str)>> Parser<'a, I> {
    fn new(tokens: I) -> Self {
        Self {
            builder: rowan::GreenNodeBuilder::new(),

            tokens: tokens.peekmore(),
            errors: Vec::new(),

            offset: 0.into(),
            depth: 0,
        }
    }

    fn peek_direct(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_nth(&mut self, n: usize) -> Option<Kind> {
        let mut peek_index = 0;
        let mut index = 0;

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

    fn peek_expecting(&mut self, expected: EnumSet<Kind>) -> ExpectMust {
        self.peek()
            .ok_or_else(|| {
                ParseError::Unexpected {
                    got: None,
                    expected,
                    at: rowan::TextRange::empty(self.offset),
                }
            })
            .into()
    }

    fn next_direct(&mut self) -> ExpectMust {
        self.tokens
            .next()
            .map(|(kind, slice)| {
                self.offset += rowan::TextSize::of(slice);
                self.builder.token(Language::kind_to_raw(kind), slice);
                kind
            })
            .ok_or_else(|| {
                ParseError::Unexpected {
                    got: None,
                    expected: EnumSet::EMPTY,
                    at: rowan::TextRange::empty(self.offset),
                }
            })
            .into()
    }

    fn next_direct_while(&mut self, predicate: impl Fn(Kind) -> bool) {
        while self.peek_direct().map_or(false, &predicate) {
            self.next_direct().unwrap();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_direct_while(Kind::is_trivia)
    }

    fn next(&mut self) -> ExpectMust {
        self.next_while_trivia();
        self.next_direct()
    }

    fn next_while(&mut self, predicate: impl Fn(Kind) -> bool) {
        while self.peek().map_or(false, &predicate) {
            self.next().unwrap();
        }
    }

    fn expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> Expect {
        let checkpoint = self.checkpoint();

        match self.peek_expecting(expected)? {
            next if expected.contains(next) => self.next().maybe(),

            got => {
                // I don't know why I don't have to run next_while_trivia here to not capture
                // the trivia for the error offsets, and I do not care anymore.
                let start = self.offset;

                self.node_from(checkpoint, NODE_ERROR, |this| {
                    this.next_while(|kind| !(until | expected).contains(kind));
                });

                let error = ParseError::Unexpected {
                    got: Some(got),
                    expected,
                    at: rowan::TextRange::new(start, self.offset),
                };

                if self.peek().map_or(false, |kind| expected.contains(kind)) {
                    log::trace!("found expected kind");
                    self.errors.push(error);
                    self.next().maybe()
                } else if let Some(peek) = self.peek() {
                    log::trace!("reached expect bound, not consuming {peek:?}",);
                    self.errors.push(error);
                    Expect::Recoverable
                } else {
                    log::trace!("expect consumed everything");
                    Expect::Deadly(error)
                }
            },
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    fn node<T>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> T) -> T {
        self.builder.start_node(Language::kind_to_raw(kind));

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn node_failable<K>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> Expect<K>) {
        let checkpoint = self.checkpoint();

        if let Expect::Deadly(error) = self.node(kind, closure) {
            self.node_from(checkpoint, NODE_ERROR, |_| {});
            self.errors.push(error);
        }
    }

    fn node_from<T>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> T,
    ) -> T {
        self.builder
            .start_node_at(checkpoint, Language::kind_to_raw(kind));

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn node_failable_from<K>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Expect<K>,
    ) {
        if let Expect::Deadly(error) = self.node_from(checkpoint, kind, closure) {
            self.node_from(checkpoint, NODE_ERROR, |_| {});
            self.errors.push(error);
        }
    }

    fn parse_expression(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        self.parse_expression_binding_power(0, until)
    }

    fn parse_expression_binding_power(
        &mut self,
        minimum_power: u16,
        until: EnumSet<Kind>,
    ) -> Expect<()> {
        let checkpoint = self.checkpoint();

        self.parse_expression_application(until)?;

        loop {
            let Some(next) = self.peek() else { break };

            let Ok((left_power, right_power)) =
                InfixOperator::try_from(next).map(|operator| operator.binding_power())
            else {
                break;
            };

            if left_power < minimum_power {
                break;
            }

            self.node_failable_from(checkpoint, NODE_INFIX_OPERATION, |this| {
                this.next().unwrap();
                this.parse_expression_binding_power(right_power, until)
            });
        }

        Expect::Found(())
    }

    fn parse_expression_application(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        let checkpoint = self.checkpoint();

        self.parse_expression_single(until)?;

        while self.peek().map_or(false, Kind::is_argument) {
            self.node_failable_from(checkpoint, NODE_APPLICATION, |this| {
                this.parse_expression_single(until)
            });
        }

        Expect::Found(())
    }

    fn parse_expression_single(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        if self.depth >= 512 {
            let error = ParseError::RecursionLimitExceeded { at: self.offset };

            self.node(NODE_ERROR, |this| {
                this.next_direct_while(|_| true);
            });

            return Expect::Deadly(error);
        }

        self.depth += 1;

        let checkpoint = self.checkpoint();

        match self.peek_expecting(EXPRESSION_TOKENS)? {
            TOKEN_PLUS | TOKEN_MINUS | TOKEN_LITERAL_NOT => self.parse_prefix_operation(until),

            TOKEN_LEFT_PARENTHESIS => {
                self.parse_parenthesis(until);
            },

            TOKEN_LEFT_BRACKET => {
                self.parse_list(until);
            },

            // TODO: Peek and do lambda parameter parsing
            // *that supports stringlike identifiers for the initial identifier*.
            //
            // # Lambda pattern initials:
            // { foo,
            // { foo ?
            // { foo }
            //
            // # Attribute set initials: Anything that isn't the above.
            //
            // Seems like I'm either going to use a sublexer that doesn't actually consume
            // anything, or write code that goes in 300 indentation levels to support
            // templated identifiers in lambda patterns.
            #[rustfmt::skip]
            TOKEN_LEFT_CURLYBRACE => {
                if matches!(self.peek_nth(1), Some(TOKEN_IDENTIFIER))
                && matches!(self.peek_nth(2), Some(TOKEN_COMMA | TOKEN_QUESTIONMARK | TOKEN_RIGHT_CURLYBRACE)) {
                    self.parse_lambda_pattern(until)
                } else {
                    self.parse_attribute_set(until)
                }
            },

            kind if IDENTIFIER_TOKENS.contains(kind) => {
                let checkpoint = self.checkpoint();

                self.parse_identifier(until);

                match self.peek() {
                    Some(TOKEN_COLON) => self.parse_lambda(checkpoint, until),

                    Some(TOKEN_AT) => self.parse_bind(checkpoint, until),

                    _ => {},
                }
            },

            TOKEN_PATH => {
                self.parse_path();
            },

            TOKEN_STRING_START => {
                self.parse_stringlike(TOKEN_STRING_START, TOKEN_STRING_END);
            },

            TOKEN_ISLAND_START => {
                self.parse_stringlike(TOKEN_ISLAND_START, TOKEN_ISLAND_END);
            },

            TOKEN_INTEGER | TOKEN_FLOAT => {
                self.parse_number(until);
            },

            TOKEN_LITERAL_IF => {
                self.parse_if_else(until);
            },

            got => {
                // TODO: Find a way to merge this with expect?
                self.next_while_trivia();
                let start = self.offset;

                self.node(NODE_ERROR, |this| {
                    this.next_while(|kind| !(until | EXPRESSION_TOKENS).contains(kind));
                });

                let error = ParseError::Unexpected {
                    got: Some(got),
                    expected: EXPRESSION_TOKENS,
                    at: rowan::TextRange::new(start, self.offset),
                };

                return if self
                    .peek()
                    .map_or(false, |kind| EXPRESSION_TOKENS.contains(kind))
                {
                    self.errors.push(error);
                    self.parse_expression_single(until)
                } else if let Some(peek) = self.peek() {
                    log::trace!("reached expect bound, not consuming {peek:?}",);
                    self.errors.push(error);
                    Expect::Recoverable
                } else {
                    log::trace!("expect consumed everything");
                    Expect::Deadly(error)
                };
            },
        }

        while let Some(TOKEN_PERIOD) = self.peek() {
            self.node_failable_from(checkpoint, NODE_ATTRIBUTE_SELECT, |this| {
                this.next().unwrap();
                this.parse_identifier(until | TOKEN_LITERAL_OR | EXPRESSION_TOKENS);

                if let Some(TOKEN_LITERAL_OR) = this.peek() {
                    this.next().unwrap();
                    this.parse_expression(until)?;
                }

                Expect::Found(())
            });
        }

        self.depth -= 1;
        Expect::Found(())
    }

    fn parse_parenthesis(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_PARENTHESIS, |this| {
            this.expect(
                TOKEN_LEFT_PARENTHESIS.into(),
                until | EXPRESSION_TOKENS | TOKEN_RIGHT_PARENTHESIS,
            )?;

            this.parse_expression(until | TOKEN_RIGHT_PARENTHESIS)?;

            this.expect(TOKEN_RIGHT_PARENTHESIS.into(), until)
        });
    }

    fn parse_bind(&mut self, checkpoint: rowan::Checkpoint, until: EnumSet<Kind>) {
        self.node_failable_from(checkpoint, NODE_BIND, |this| {
            this.expect(TOKEN_AT.into(), until | EXPRESSION_TOKENS)?;
            this.parse_expression_single(until)
        });
    }

    fn parse_list(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_LIST, |this| {
            this.expect(
                TOKEN_LEFT_BRACKET.into(),
                until | EXPRESSION_TOKENS | TOKEN_RIGHT_BRACKET,
            )?;

            while this.peek_expecting(EXPRESSION_TOKENS | TOKEN_RIGHT_BRACKET)?
                != TOKEN_RIGHT_BRACKET
            {
                this.parse_expression_single(until | TOKEN_RIGHT_BRACKET)?;
            }

            this.expect(TOKEN_RIGHT_BRACKET.into(), until)
        });
    }

    fn parse_attribute_set(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_ATTRIBUTE_SET, |this| {
            this.expect(TOKEN_LEFT_CURLYBRACE.into(), until | TOKEN_RIGHT_CURLYBRACE)?;

            while !(until | TOKEN_RIGHT_CURLYBRACE)
                .contains(this.peek_expecting(IDENTIFIER_TOKENS | TOKEN_RIGHT_CURLYBRACE)?)
            {
                this.parse_attribute(until | TOKEN_RIGHT_CURLYBRACE)?;
            }

            this.expect(TOKEN_RIGHT_CURLYBRACE.into(), until)
        });
    }

    fn parse_attribute(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a semicolon,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a period or
        // an equals, this is a NODE_ATTRIBUTE.
        self.parse_identifier(
            until | TOKEN_EQUAL | TOKEN_PERIOD | EXPRESSION_TOKENS | TOKEN_SEMICOLON,
        );

        if self.peek_expecting(TOKEN_PERIOD | TOKEN_EQUAL | TOKEN_SEMICOLON)? == TOKEN_SEMICOLON {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                this.next().unwrap();
            });
        } else {
            self.node_from(checkpoint, NODE_ATTRIBUTE, |this| {
                this.node_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_expecting(TOKEN_PERIOD | TOKEN_EQUAL)? == TOKEN_PERIOD {
                        this.next().unwrap();
                        this.parse_identifier(
                            until | TOKEN_EQUAL | EXPRESSION_TOKENS | TOKEN_SEMICOLON,
                        );
                    }
                    Expect::Found(())
                })?;

                this.expect(
                    TOKEN_EQUAL.into(),
                    until | EXPRESSION_TOKENS | TOKEN_SEMICOLON,
                )?;
                this.parse_expression(until | TOKEN_SEMICOLON)?;
                this.expect(TOKEN_SEMICOLON.into(), until)
            })?;
        }

        Expect::Found(())
    }

    fn parse_lambda(&mut self, checkpoint: rowan::Checkpoint, until: EnumSet<Kind>) {
        self.node_failable_from(checkpoint, NODE_LAMBDA, |this| {
            this.node_from(checkpoint, NODE_LAMBDA_PARAMETER_IDENTIFIER, |_| {});

            this.expect(TOKEN_COLON.into(), until | EXPRESSION_TOKENS)?;

            this.parse_expression(until)
        })
    }

    fn parse_lambda_pattern(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_LAMBDA, |this| {
            this.node(NODE_LAMBDA_PARAMETER_PATTERN, |this| {
                this.expect(
                    TOKEN_LEFT_CURLYBRACE.into(),
                    until
                        | IDENTIFIER_TOKENS
                        | TOKEN_COMMA
                        | TOKEN_QUESTIONMARK
                        | EXPRESSION_TOKENS
                        | TOKEN_RIGHT_CURLYBRACE
                        | TOKEN_COLON,
                )?;

                while !(until | TOKEN_RIGHT_CURLYBRACE)
                    .contains(this.peek_expecting(IDENTIFIER_TOKENS | TOKEN_RIGHT_CURLYBRACE)?)
                {
                    if let Expect::Found(false) = this.parse_lambda_pattern_attribute(
                        until | TOKEN_RIGHT_CURLYBRACE | TOKEN_COLON,
                    ) {
                        break;
                    }
                }

                this.expect(
                    TOKEN_RIGHT_CURLYBRACE.into(),
                    until | TOKEN_COLON | EXPRESSION_TOKENS,
                )
            })?;

            this.expect(TOKEN_COLON.into(), until | EXPRESSION_TOKENS)?;

            this.parse_expression(until)
        });
    }

    fn parse_lambda_pattern_attribute(&mut self, until: EnumSet<Kind>) -> Expect<bool> {
        self.node(NODE_LAMBDA_PARAMETER_PATTERN_ATTRIBUTE, |this| {
            this.parse_identifier(
                until
                    | TOKEN_COMMA
                    | TOKEN_QUESTIONMARK
                    | EXPRESSION_TOKENS
                    | TOKEN_RIGHT_CURLYBRACE,
            );

            if this.peek_expecting(TOKEN_COMMA | TOKEN_QUESTIONMARK | TOKEN_RIGHT_CURLYBRACE)?
                == TOKEN_QUESTIONMARK
            {
                this.next().unwrap();
                this.parse_expression(until)?;
            }

            if this.peek_expecting(TOKEN_COMMA | TOKEN_RIGHT_BRACKET)? != TOKEN_RIGHT_BRACKET {
                this.expect(TOKEN_COMMA.into(), until)?;
                Expect::Found(true)
            } else {
                Expect::Found(false)
            }
        })
    }

    fn parse_prefix_operation(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_PREFIX_OPERATION, |this| {
            this.expect(
                TOKEN_PLUS | TOKEN_MINUS | TOKEN_LITERAL_NOT,
                until | EXPRESSION_TOKENS,
            )?;
            this.parse_expression(until)
        });
    }

    fn parse_path(&mut self) {
        self.node_failable(NODE_PATH, |this| {
            loop {
                match this.peek_direct() {
                    Some(TOKEN_PATH) => {
                        this.next_direct().unwrap();
                    },

                    Some(TOKEN_INTERPOLATION_START) => {
                        this.parse_interpolation()?;
                    },

                    _ => break Expect::Found(()),
                }
            }
        });
    }

    fn parse_identifier(&mut self, until: EnumSet<Kind>) {
        if self.peek() == Some(TOKEN_IDENTIFIER_START) {
            self.parse_stringlike(TOKEN_IDENTIFIER_START, TOKEN_IDENTIFIER_END);
        } else {
            self.node_failable(NODE_IDENTIFIER, |this| {
                this.expect(IDENTIFIER_TOKENS, until)
            });
        }
    }

    fn parse_stringlike(&mut self, start: Kind, end: Kind) {
        self.node_failable(
            match start {
                TOKEN_IDENTIFIER_START => NODE_IDENTIFIER,
                TOKEN_STRING_START => NODE_STRING,
                TOKEN_ISLAND_START => NODE_ISLAND,
                _ => unreachable!(),
            },
            |this| {
                let current = this.next();
                debug_assert_eq!(current, ExpectMust::Found(start));

                loop {
                    match this.peek_direct() {
                        Some(TOKEN_CONTENT) => {
                            this.next_direct().unwrap();
                        },

                        Some(TOKEN_INTERPOLATION_START) => {
                            this.parse_interpolation()?;
                        },

                        Some(other) if other == end => {
                            this.next_direct().unwrap();
                            break Expect::Found(());
                        },

                        _ => unreachable!(),
                    }
                }
            },
        );
    }

    fn parse_interpolation(&mut self) -> Expect<()> {
        self.node(NODE_INTERPOLATION, |this| {
            this.expect(TOKEN_INTERPOLATION_START.into(), EnumSet::EMPTY)
                .must()?;

            this.parse_expression(TOKEN_RIGHT_CURLYBRACE.into())?;

            this.expect(TOKEN_INTERPOLATION_END.into(), EnumSet::EMPTY)
                .must()?;

            Expect::Found(())
        })
    }

    fn parse_number(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_NUMBER, |this| {
            this.expect(TOKEN_INTEGER | TOKEN_FLOAT, until)
        });
    }

    fn parse_if_else(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_IF_ELSE, |this| {
            this.expect(
                TOKEN_LITERAL_IF.into(),
                until | EXPRESSION_TOKENS | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
            )?;
            this.parse_expression(until | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE)?;

            this.expect(TOKEN_LITERAL_THEN.into(), until | TOKEN_LITERAL_ELSE)?;
            this.parse_expression(until | TOKEN_LITERAL_ELSE)?;

            if this.peek() == Some(TOKEN_LITERAL_ELSE) {
                this.next().unwrap();
                this.parse_expression(until)?;
            }
            Expect::Found(())
        });
    }
}

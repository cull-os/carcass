use std::{
    convert::Infallible,
    fmt,
    ops::{
        ControlFlow,
        FromResidual,
        Try,
    },
    panic::Location,
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
    node::Root,
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

    fn assert(self) -> K {
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

                if expected.is_superset(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START) {
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
        if let Expect::Deadly(error) = this.parse_expression_until(EnumSet::EMPTY) {
            log::trace!("unrecoverable error encountered: {error:?}");

            this.node_from(checkpoint, NODE_ERROR, |_| {});

            this.errors.push(error);
        }

        if let Some(got) = this.peek() {
            log::trace!("leftovers encountered: {got:?}");

            let start = this.offset;

            this.node(NODE_ERROR, |this| {
                this.next_direct_while(|_| true);
            });

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
            match self.tokens.peek_nth(peek_index) {
                None => return None,

                Some(&(kind, _)) => {
                    if !kind.is_trivia() {
                        index += 1;
                    }

                    // If it is trivia or we haven't reached the desired real index,
                    // move on to the next iteration.
                    if kind.is_trivia() || index < n {
                        peek_index += 1;
                        continue;
                    }

                    return Some(kind);
                },
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
            self.next_direct().assert();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_direct_while(Kind::is_trivia)
    }

    fn next(&mut self) -> ExpectMust {
        self.next_while_trivia();
        self.next_direct()
    }

    fn expect_until(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> Expect {
        let checkpoint = self.checkpoint();

        match self.peek() {
            Some(next) if expected.contains(next) => self.next().maybe(),

            Some(got) => {
                let start = self.offset;

                self.node_from(checkpoint, NODE_ERROR, |this| {
                    this.next_direct_while(|kind| {
                        !expected.contains(kind) && !until.contains(kind)
                    });
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

            None => {
                Expect::Deadly(ParseError::Unexpected {
                    got: None,
                    expected,
                    at: rowan::TextRange::empty(self.offset),
                })
            },
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    #[track_caller]
    fn node<T>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> T) -> T {
        log::trace!(
            "starting node {kind:?} in {location}",
            location = Location::caller()
        );
        self.builder.start_node(Language::kind_to_raw(kind));

        let result = closure(self);

        log::trace!("ending node at {location}", location = Location::caller());
        self.builder.finish_node();
        result
    }

    #[track_caller]
    #[allow(unused)]
    fn node_failable<K>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> Expect<K>) {
        let checkpoint = self.checkpoint();

        if let Expect::Deadly(error) = self.node(kind, closure) {
            self.errors.push(error);
            self.node_from(checkpoint, NODE_ERROR, |_| {});
        }
    }

    #[track_caller]
    fn node_from<T>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> T,
    ) -> T {
        log::trace!(
            "starting node {kind:?} at {checkpoint:?} in {location}",
            location = Location::caller()
        );
        self.builder
            .start_node_at(checkpoint, Language::kind_to_raw(kind));

        let result = closure(self);

        log::trace!("ending node at {location}", location = Location::caller());
        self.builder.finish_node();
        result
    }

    #[track_caller]
    fn node_failable_from<K>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Expect<K>,
    ) {
        if let Expect::Deadly(error) = self.node_from(checkpoint, kind, closure) {
            self.errors.push(error);
            self.node_from(checkpoint, NODE_ERROR, |_| {});
        }
    }

    fn parse_stringlike_inner(&mut self, end: Kind) -> Expect<()> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let checkpoint = self.checkpoint();

            let current = self.expect_until(
                TOKEN_CONTENT | TOKEN_INTERPOLATION_START | end,
                EnumSet::EMPTY,
            )?;

            if current == Some(TOKEN_INTERPOLATION_START) {
                self.node_from(checkpoint, NODE_INTERPOLATION, |this| {
                    this.parse_expression_until(TOKEN_INTERPOLATION_END.into())?;
                    this.expect_until(TOKEN_INTERPOLATION_END.into(), EnumSet::EMPTY)
                })?;
            } else if current == Some(end) {
                break;
            }
        }

        Expect::Found(())
    }

    fn parse_identifier_until(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        self.node(NODE_IDENTIFIER, |this| {
            // If it is a normal identifier, we don't do anything
            // else as it only has a single token as .expect_until() consumes it.
            if this
                .expect_until(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START, until)
                .must()?
                == TOKEN_IDENTIFIER_START
            {
                this.parse_stringlike_inner(TOKEN_IDENTIFIER_END)?;
            }

            Expect::Found(())
        })
    }

    fn parse_attribute_until(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a semicolon,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a period or
        // an equals, this is a NODE_ATTRIBUTE.
        self.parse_identifier_until(
            until | TOKEN_EQUAL | TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE,
        )?;

        if self.peek_expecting(TOKEN_SEMICOLON | TOKEN_PERIOD | TOKEN_EQUAL)? == TOKEN_SEMICOLON {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                this.next().assert();
            });
        } else {
            self.node_failable_from(checkpoint, NODE_ATTRIBUTE, |this| {
                this.node_failable_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_expecting(TOKEN_PERIOD | TOKEN_EQUAL)? == TOKEN_PERIOD {
                        this.next().assert();
                        this.parse_identifier_until(
                            until | TOKEN_EQUAL | TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE,
                        )?;
                    }
                    Expect::Found(())
                });

                this.expect_until(TOKEN_EQUAL.into(), EXPRESSION_TOKENS)?;
                this.parse_expression_until(TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE)?;
                this.expect_until(TOKEN_SEMICOLON.into(), TOKEN_RIGHT_CURLYBRACE.into())
            });
        }

        Expect::Found(())
    }

    fn parse_interpolation(&mut self) -> Expect<()> {
        self.node(NODE_INTERPOLATION, |this| {
            this.expect_until(TOKEN_INTERPOLATION_START.into(), EnumSet::EMPTY)?;
            this.parse_expression_until(TOKEN_RIGHT_CURLYBRACE.into())?;
            this.expect_until(TOKEN_INTERPOLATION_END.into(), EnumSet::EMPTY)?;
            Expect::Found(())
        })
    }

    fn parse_expression_until(&mut self, until: EnumSet<Kind>) -> Expect<()> {
        if self.depth >= 512 {
            let error = ParseError::RecursionLimitExceeded { at: self.offset };

            self.node(NODE_ERROR, |this| {
                this.next_direct_while(|_| true);
            });

            return Expect::Deadly(error);
        }

        self.depth += 1;

        let checkpoint = self.checkpoint();

        match self.expect_until(EXPRESSION_TOKENS, until)? {
            Some(TOKEN_LEFT_PARENTHESIS) => {
                self.node_failable_from(checkpoint, NODE_PARENTHESIS, |this| {
                    this.parse_expression_until(until | TOKEN_RIGHT_PARENTHESIS)?;
                    this.expect_until(TOKEN_RIGHT_PARENTHESIS.into(), until)
                })
            },

            Some(TOKEN_LEFT_BRACKET) => {
                self.node_failable_from(checkpoint, NODE_LIST, |this| {
                    while {
                        let peek = this.peek_expecting(TOKEN_RIGHT_BRACKET | EXPRESSION_TOKENS)?;
                        !until.contains(peek) && peek != TOKEN_RIGHT_BRACKET
                    } {
                        // TODO: Seperate expression parsing logic into two functions
                        // to not parse multiple expressions next to eachother as an
                        // application chain.
                        this.parse_expression_until(until | TOKEN_RIGHT_BRACKET)?;
                    }

                    this.expect_until(TOKEN_RIGHT_BRACKET.into(), until)
                });
            },

            // TODO: Peek and do lambda parameter parsing.
            //
            // # Attribute set initials:
            // {
            // { foo
            // { foo;
            // { foo =
            //
            // # Lambda pattern initials:
            // { foo,
            // { foo ?
            Some(TOKEN_LEFT_CURLYBRACE) => {
                self.node_failable_from(checkpoint, NODE_ATTRIBUTE_SET, |this| {
                    while this.peek_expecting(
                        TOKEN_RIGHT_CURLYBRACE | TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START,
                    )? != TOKEN_RIGHT_CURLYBRACE
                    {
                        this.parse_attribute_until(until | TOKEN_RIGHT_CURLYBRACE)?;
                    }

                    this.expect_until(TOKEN_RIGHT_CURLYBRACE.into(), until)
                });
            },

            Some(TOKEN_PLUS | TOKEN_MINUS | TOKEN_LITERAL_NOT) => {
                self.node_failable_from(checkpoint, NODE_PREFIX_OPERATION, |this| {
                    this.parse_expression_until(until)
                });
            },

            Some(TOKEN_PATH) => {
                self.node_failable_from(checkpoint, NODE_PATH, |this| {
                    loop {
                        let peek = this.peek();

                        if peek == Some(TOKEN_INTERPOLATION_START) {
                            this.parse_interpolation()?;
                        } else if peek == Some(TOKEN_PATH) {
                            this.next().assert();
                        } else {
                            break;
                        }
                    }
                    Expect::Found(())
                });
            },

            Some(start @ (TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START)) => {
                let identifier_parse_result = if start == TOKEN_IDENTIFIER {
                    Expect::Found(())
                } else {
                    self.parse_stringlike_inner(TOKEN_IDENTIFIER_END)
                };

                if self.peek() == Some(TOKEN_COLON) {
                    self.node_failable_from(checkpoint, NODE_LAMBDA, |this| {
                        this.node_from(checkpoint, NODE_LAMBDA_PARAMETER_IDENTIFIER, |this| {
                            this.node_from(checkpoint, NODE_IDENTIFIER, |_| identifier_parse_result)
                        })?;

                        this.next().assert();

                        this.parse_expression_until(until)
                    });
                } else {
                    self.node_failable_from(checkpoint, NODE_IDENTIFIER, |_| {
                        identifier_parse_result
                    });
                }
            },

            Some(TOKEN_STRING_START) => {
                self.node_failable_from(checkpoint, NODE_STRING, |this| {
                    this.parse_stringlike_inner(TOKEN_STRING_END)
                });
            },

            Some(TOKEN_ISLAND_START) => {
                self.node_failable_from(checkpoint, NODE_ISLAND, |this| {
                    this.parse_stringlike_inner(TOKEN_ISLAND_END)
                });
            },

            Some(TOKEN_INTEGER | TOKEN_FLOAT) => {
                self.node_from(checkpoint, NODE_NUMBER, |_| {});
            },

            Some(TOKEN_LITERAL_IF) => {
                self.node_failable_from(checkpoint, NODE_IF_ELSE, |this| {
                    this.parse_expression_until(until | TOKEN_LITERAL_THEN)?;
                    this.expect_until(TOKEN_LITERAL_THEN.into(), until)?;
                    this.parse_expression_until(until | TOKEN_LITERAL_ELSE)?;

                    if this.peek() == Some(TOKEN_LITERAL_ELSE) {
                        this.next().assert();
                        this.parse_expression_until(until)?;
                    }
                    Expect::Found(())
                });
            },

            None => {
                self.node_from(checkpoint, NODE_ERROR, |_| {});
            },

            _ => unreachable!(),
        }

        self.depth -= 1;
        Expect::Found(())
    }
}

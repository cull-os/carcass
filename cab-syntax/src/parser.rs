use std::{
    fmt,
    iter::Peekable,
    panic::Location,
};

use enumset::EnumSet;
use peekmore::PeekMore;
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

macro_rules! EXPRESSION_TOKENS {
    () => {
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
    };
}

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
        expected: Option<EnumSet<Kind>>,
        /// The range that contains the unexpected token sequence.
        at: rowan::TextRange,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn format_enumset(
            mut set: EnumSet<Kind>,
            formatter: &mut fmt::Formatter<'_>,
        ) -> fmt::Result {
            if set & EXPRESSION_TOKENS!() == EXPRESSION_TOKENS!() {
                write!(formatter, "an expression")?;

                set = set.difference(EXPRESSION_TOKENS!());

                match set.iter().count() {
                    0 => {},
                    1 => write!(formatter, " or ")?,
                    2.. => write!(formatter, ", ")?,
                }
            }

            if set.contains(TOKEN_IDENTIFIER) && set.contains(TOKEN_IDENTIFIER_START) {
                set &= !TOKEN_IDENTIFIER_START
            }

            let mut iterator = set.iter().peekmore();
            while let Some(item) = iterator.next() {
                write!(
                    formatter,
                    "{item}{seperator}",
                    seperator = match (iterator.peek().is_some(), iterator.peek_nth(1).is_some()) {
                        (false, false) => "",
                        (true, false) => " or ",
                        (true, true) => ", ",
                        _ => unreachable!(),
                    }
                )?;
            }

            Ok(())
        }

        match self {
            Self::RecursionLimitExceeded { .. } => write!(formatter, "recursion limit exceeded"),

            Self::Unexpected {
                got: None,
                expected: Some(expected),
                at,
            } => {
                assert_eq!(at.start(), at.end());

                write!(formatter, "expected ")?;
                format_enumset(*expected, formatter)?;
                write!(formatter, ", reached end of file")
            },

            Self::Unexpected {
                got: Some(got),
                expected: None,
                ..
            } => {
                write!(formatter, "expected end of file, got {got}")
            },

            Self::Unexpected {
                got: Some(got),
                expected: Some(expected),
                ..
            } => {
                write!(formatter, "expected ")?;
                format_enumset(*expected, formatter)?;
                write!(formatter, ", got {got}")
            },

            other => unreachable!("unhandled ParseError format: {other:?}"),
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
        if let Err(error) = this.parse_expression_until(EnumSet::EMPTY) {
            log::trace!("unrecoverable error encountered: {error:?}");

            this.node_from(checkpoint, NODE_ERROR, |_| {});

            this.errors.push(error);
        }

        if let Some(got) = this.peek_nontrivia() {
            log::trace!("leftovers encountered: {got:?}");

            let start = this.offset;

            this.node(NODE_ERROR, |this| {
                this.next_while(|_| true);
            });

            this.errors.push(ParseError::Unexpected {
                got: Some(got),
                expected: None,
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

    tokens: Peekable<I>,
    errors: Vec<ParseError>,

    offset: rowan::TextSize,
    depth: u32,
}

impl<'a, I: Iterator<Item = (Kind, &'a str)>> Parser<'a, I> {
    fn new(tokens: I) -> Self {
        Self {
            builder: rowan::GreenNodeBuilder::new(),

            tokens: tokens.peekable(),
            errors: Vec::new(),

            offset: 0.into(),
            depth: 0,
        }
    }

    fn peek(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_nontrivia(&mut self) -> Option<Kind> {
        self.next_while(Kind::is_trivia);
        self.peek()
    }

    fn peek_nontrivia_expecting(&mut self, expected: EnumSet<Kind>) -> Result<Kind, ParseError> {
        self.peek_nontrivia().ok_or_else(|| {
            ParseError::Unexpected {
                got: None,
                expected: Some(expected),
                at: rowan::TextRange::empty(self.offset),
            }
        })
    }

    fn next(&mut self) -> Result<Kind, ParseError> {
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
                    expected: None,
                    at: rowan::TextRange::empty(self.offset),
                }
            })
    }

    #[allow(unused)]
    fn next_nontrivia(&mut self) -> Result<Kind, ParseError> {
        self.next_while_trivia();
        self.next()
    }

    fn next_while(&mut self, predicate: impl Fn(Kind) -> bool) {
        while self.peek().map_or(false, &predicate) {
            self.next().unwrap();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_while(Kind::is_trivia)
    }

    fn expect(&mut self, expected: EnumSet<Kind>) -> Result<Kind, ParseError> {
        self.expect_until(expected, EnumSet::EMPTY)
            .map(Option::unwrap)
    }

    fn expect_until(
        &mut self,
        expected: EnumSet<Kind>,
        until: EnumSet<Kind>,
    ) -> Result<Option<Kind>, ParseError> {
        let checkpoint = self.checkpoint();

        match self.peek_nontrivia() {
            Some(next) if expected.contains(next) => Ok(Some(self.next().unwrap())),

            Some(got) => {
                let start = self.offset;

                self.node_from(checkpoint, NODE_ERROR, |this| {
                    this.next_while(|kind| !expected.contains(kind) && !until.contains(kind));
                });

                let error = ParseError::Unexpected {
                    got: Some(got),
                    expected: Some(expected),
                    at: rowan::TextRange::new(start, self.offset),
                };

                if self
                    .peek_nontrivia()
                    .map_or(false, |kind| expected.contains(kind))
                {
                    log::trace!("found expected kind");
                    self.errors.push(error);
                    Ok(Some(self.next().unwrap()))
                } else if let Some(peek) = self.peek() {
                    log::trace!("reached expect bound, not consuming {peek:?}",);
                    self.errors.push(error);
                    Ok(None)
                } else {
                    log::trace!("expect consumed everything");
                    Err(error)
                }
            },

            None => {
                Err(ParseError::Unexpected {
                    got: None,
                    expected: Some(expected),
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
    fn node_failable(
        &mut self,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), Option<ParseError>>,
    ) {
        let checkpoint = self.checkpoint();

        if let Err(error) = self.node(kind, closure) {
            if let Some(error) = error {
                self.errors.push(error);
            }

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
    fn node_failable_from(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), ParseError>,
    ) {
        if let Err(error) = self.node_from(checkpoint, kind, closure) {
            self.errors.push(error);
            self.node_from(checkpoint, NODE_ERROR, |_| {});
        }
    }

    fn parse_stringlike_inner(&mut self, end: Kind) -> Result<(), ParseError> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let checkpoint = self.checkpoint();

            let current = self.expect(TOKEN_CONTENT | TOKEN_INTERPOLATION_START | end)?;

            if current == TOKEN_INTERPOLATION_START {
                self.node_from(checkpoint, NODE_INTERPOLATION, |this| {
                    this.parse_expression_until(TOKEN_INTERPOLATION_END.into())?;
                    this.expect(TOKEN_INTERPOLATION_END.into())?;
                    Ok(())
                })?;
            } else if current == end {
                break;
            }
        }

        Ok(())
    }

    fn parse_identifier_until(&mut self, until: EnumSet<Kind>) {
        self.node_failable(NODE_IDENTIFIER, |this| {
            // If it is a normal identifier, we don't do anything
            // else as it only has a single token as .expect_until() consumes it.
            if this
                .expect_until(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START, until)?
                .ok_or(None)?
                == TOKEN_IDENTIFIER_START
            {
                this.parse_stringlike_inner(TOKEN_IDENTIFIER_END)?;
            }
            Ok(())
        })
    }

    fn parse_attribute_until(&mut self, until: EnumSet<Kind>) -> Result<(), ParseError> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a semicolon,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a period or
        // an equals, this is a NODE_ATTRIBUTE.
        self.parse_identifier_until(until | TOKEN_EQUAL | TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE);

        if self.peek_nontrivia_expecting(TOKEN_SEMICOLON | TOKEN_PERIOD | TOKEN_EQUAL)?
            == TOKEN_SEMICOLON
        {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                this.next().unwrap();
            });
        } else {
            self.node_failable_from(checkpoint, NODE_ATTRIBUTE, |this| {
                this.node_failable_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_nontrivia_expecting(TOKEN_PERIOD | TOKEN_EQUAL)? == TOKEN_PERIOD
                    {
                        this.next().unwrap();
                        this.parse_identifier_until(
                            until | TOKEN_EQUAL | TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE,
                        );
                    }
                    Ok(())
                });

                this.expect_until(TOKEN_EQUAL.into(), EXPRESSION_TOKENS!())?;
                this.parse_expression_until(TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE)?;
                this.expect_until(TOKEN_SEMICOLON.into(), TOKEN_RIGHT_CURLYBRACE.into())?;
                Ok(())
            });
        }

        Ok(())
    }

    fn parse_interpolation(&mut self) -> Result<(), ParseError> {
        self.node(NODE_INTERPOLATION, |this| {
            this.expect(TOKEN_INTERPOLATION_START.into())?;
            this.parse_expression_until(TOKEN_RIGHT_CURLYBRACE.into())?;
            this.expect(TOKEN_INTERPOLATION_END.into())?;
            Ok(())
        })
    }

    fn parse_expression_until(&mut self, until: EnumSet<Kind>) -> Result<(), ParseError> {
        if self.depth >= 512 {
            let error = ParseError::RecursionLimitExceeded { at: self.offset };

            self.node(NODE_ERROR, |this| {
                this.next_while(|_| true);
            });

            return Err(error);
        }

        self.depth += 1;

        let checkpoint = self.checkpoint();

        match self.expect_until(EXPRESSION_TOKENS!(), until)? {
            Some(TOKEN_LEFT_PARENTHESIS) => {
                self.node_failable_from(checkpoint, NODE_PARENTHESIS, |this| {
                    this.parse_expression_until(until | TOKEN_RIGHT_PARENTHESIS)?;
                    this.expect_until(TOKEN_RIGHT_PARENTHESIS.into(), until)?;
                    Ok(())
                })
            },

            Some(TOKEN_LEFT_BRACKET) => {
                self.node_failable_from(checkpoint, NODE_LIST, |this| {
                    while {
                        let peek = this
                            .peek_nontrivia_expecting(TOKEN_RIGHT_BRACKET | EXPRESSION_TOKENS!())?;
                        !until.contains(peek) && peek != TOKEN_RIGHT_BRACKET
                    } {
                        // TODO: Seperate expression parsing logic into two functions
                        // to not parse multiple expressions next to eachother as an
                        // application chain.
                        this.parse_expression_until(until | TOKEN_RIGHT_BRACKET)?;
                    }

                    this.expect_until(TOKEN_RIGHT_BRACKET.into(), until)?;
                    Ok(())
                });
            },

            // TODO: Peek and do lambda parameter parsing.
            Some(TOKEN_LEFT_CURLYBRACE) => {
                self.node_failable_from(checkpoint, NODE_ATTRIBUTE_SET, |this| {
                    while this.peek_nontrivia_expecting(
                        TOKEN_RIGHT_CURLYBRACE | TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START,
                    )? != TOKEN_RIGHT_CURLYBRACE
                    {
                        this.parse_attribute_until(until | TOKEN_RIGHT_CURLYBRACE)?;
                    }

                    this.expect_until(TOKEN_RIGHT_CURLYBRACE.into(), until)?;
                    Ok(())
                });
            },

            Some(TOKEN_PLUS | TOKEN_MINUS | TOKEN_LITERAL_NOT) => {
                self.node_failable_from(checkpoint, NODE_PREFIX_OPERATION, |this| {
                    this.parse_expression_until(until)?;
                    Ok(())
                });
            },

            Some(TOKEN_PATH) => {
                self.node_failable_from(checkpoint, NODE_PATH, |this| {
                    loop {
                        let peek = this.peek_nontrivia();

                        if peek == Some(TOKEN_INTERPOLATION_START) {
                            this.parse_interpolation()?;
                        } else if peek == Some(TOKEN_PATH) {
                            this.next().unwrap();
                        } else {
                            break;
                        }
                    }
                    Ok(())
                });
            },

            Some(start @ (TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START)) => {
                let identifier_parse_result = if start == TOKEN_IDENTIFIER {
                    Ok(())
                } else {
                    self.parse_stringlike_inner(TOKEN_IDENTIFIER_END)
                };

                if self.peek_nontrivia() == Some(TOKEN_COLON) {
                    self.node_failable_from(checkpoint, NODE_LAMBDA, |this| {
                        this.node_from(checkpoint, NODE_LAMBDA_PARAMETER_IDENTIFIER, |this| {
                            this.node_from(checkpoint, NODE_IDENTIFIER, |_| {});
                        });

                        this.next().unwrap();

                        this.parse_expression_until(until)
                    });
                } else {
                    self.node_failable_from(checkpoint, NODE_IDENTIFIER, |_| {
                        identifier_parse_result?;
                        Ok(())
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

                    if this.peek_nontrivia() == Some(TOKEN_LITERAL_ELSE) {
                        this.next().unwrap();
                        this.parse_expression_until(until)?;
                    }
                    Ok(())
                });
            },

            None => {
                self.node_from(checkpoint, NODE_ERROR, |_| {});
            },

            _ => unreachable!(),
        }

        self.depth -= 1;
        Ok(())
    }
}

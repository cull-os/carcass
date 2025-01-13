use std::fmt;

use enumset::{
    enum_set,
    EnumSet,
};
use peekmore::{
    PeekMore as _,
    PeekMoreIterator as PeekMore,
};

use crate::{
    node::{
        self,
        Node,
    },
    tokenize,
    Kind::{
        self,
        *,
    },
    RowanNode,
};

type MustParseResult<T = ()> = Result<T, ParseError>;

type ParseResult<T = ()> = Result<Option<T>, ParseError>;

fn found<T>(value: T) -> ParseResult<T> {
    Ok(Some(value))
}

fn recoverable<T>() -> ParseResult<T> {
    Ok(None)
}

fn deadly<T>(error: ParseError) -> ParseResult<T> {
    Err(error)
}

const EXPRESSION_TOKENS: EnumSet<Kind> = enum_set!(
    TOKEN_LEFT_PARENTHESIS
        | TOKEN_LEFT_BRACKET
        | TOKEN_LEFT_CURLYBRACE
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

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecursionLimitExceeded { .. } => write!(formatter, "recursion limit exceeded"),

            Self::Unexpected {
                got: Some(got),
                expected: const { EnumSet::empty() },
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

/// A parse result that contains a [`rowan::SyntaxNode`],
/// contains a [`Node`] if it was successfully created,
/// and a list of [`ParseError`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse<N: node::Node> {
    /// The underlying [`rowan::SyntaxNode`].
    pub syntax: RowanNode,

    /// The [`Node`], if it was successfully created.
    pub node: Option<N>,

    errors: Vec<ParseError>,
}

impl<N: Node> Parse<N> {
    /// Returns an iterator over the underlying [`ParseError`]s, removing
    /// duplicates and only returning useful errors.
    pub fn errors(&self) -> Box<dyn Iterator<Item = &ParseError> + '_> {
        if let Some(recursion_error) = self
            .errors
            .iter()
            .find(|error| matches!(error, ParseError::RecursionLimitExceeded { .. }))
        {
            return Box::new([recursion_error].into_iter());
        }

        let extra_error_count = self
            .errors
            .iter()
            .rev()
            .take_while(|error| matches!(error, ParseError::Unexpected { got: None, .. }))
            .count() as isize
            - 1;

        Box::new(
            self.errors
                .iter()
                .take(self.errors.len() - extra_error_count.max(0) as usize),
        )
    }

    /// Returns [`Ok`] with the [`Node`] node if there are no errors,
    /// returns [`Err`] with the list of errors obtained from
    /// [`Self::errors`] otherwise.
    pub fn result(self) -> Result<N, Vec<ParseError>> {
        if self.errors.is_empty() {
            Ok(self.node.unwrap())
        } else {
            Err(self.errors().cloned().collect())
        }
    }
}

/// Parses a string reference and returns a [`Parse`].
pub fn parse<N: Node>(input: &str) -> Parse<N> {
    let mut parser = Parser::new(tokenize(input));

    parser.node(NODE_ROOT, |this| {
        let checkpoint = this.checkpoint();

        // Reached an unrecoverable error.
        if let Err(error) = this.parse_expression(EnumSet::empty()) {
            this.node_from(checkpoint, NODE_ERROR, |_| {});
            this.errors.push(error);
        }

        if let Some(unexpected) = this.peek() {
            let start = this.offset;

            this.node(NODE_ERROR, |this| this.next_direct_while(|_| true));
            this.errors.push(ParseError::Unexpected {
                got: Some(unexpected),
                expected: EnumSet::empty(),
                at: rowan::TextRange::new(start, this.offset),
            });
        }
    });

    let syntax = RowanNode::new_root(parser.builder.finish());

    let child = syntax.first_child().unwrap();
    let child_text_range = child.text_range();
    let child_kind = child.kind();

    let mut errors = parser.errors;

    let node = if let Some(expected) = N::inherent_kind() {
        let cast = N::cast(child);

        if cast.is_none() {
            errors.push(ParseError::Unexpected {
                got: Some(child_kind),
                expected: expected.into(),
                at: child_text_range,
            })
        }

        cast
    } else {
        N::cast(child)
    };

    Parse {
        syntax,
        node,
        errors,
    }
}

struct Parser<'a, I: Iterator<Item = (Kind, &'a str)>> {
    builder: rowan::GreenNodeBuilder<'a>,

    tokens: PeekMore<I>,
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

    fn node_failable<K>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> ParseResult<K>) {
        let checkpoint = self.checkpoint();

        if let Err(error) = self.node(kind, closure) {
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
        self.builder.start_node_at(checkpoint, kind.into());

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn node_failable_from<K>(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> ParseResult<K>,
    ) {
        if let Err(error) = self.node_from(checkpoint, kind, closure) {
            self.node_from(checkpoint, NODE_ERROR, |_| {});
            self.errors.push(error);
        }
    }

    fn peek_direct(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_direct_expecting(&mut self, expected: EnumSet<Kind>) -> MustParseResult<Kind> {
        self.peek_direct().ok_or(ParseError::Unexpected {
            got: None,
            expected,
            at: rowan::TextRange::empty(self.offset),
        })
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

    fn peek_expecting(&mut self, expected: EnumSet<Kind>) -> MustParseResult<Kind> {
        self.peek().ok_or(ParseError::Unexpected {
            got: None,
            expected,
            at: rowan::TextRange::empty(self.offset),
        })
    }

    fn next_direct(&mut self) -> MustParseResult<Kind> {
        self.tokens
            .next()
            .map(|(kind, slice)| {
                self.offset += rowan::TextSize::of(slice);
                self.builder.token(kind.into(), slice);
                kind
            })
            .ok_or(ParseError::Unexpected {
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

    fn next(&mut self) -> MustParseResult<Kind> {
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

    fn expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> ParseResult<Kind> {
        let checkpoint = self.checkpoint();

        match self.peek_expecting(expected)? {
            next if expected.contains(next) => self.next().map(Some),

            unexpected => {
                let start = self.offset;

                self.node_from(checkpoint, NODE_ERROR, |this| {
                    this.next_while(|kind| !(until | expected).contains(kind));
                });

                let error = ParseError::Unexpected {
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
                let Some(true) = this.parse_attribute(until | TOKEN_RIGHT_CURLYBRACE)? else {
                    break;
                };
            }

            this.expect(TOKEN_RIGHT_CURLYBRACE.into(), until)
        });
    }

    fn parse_attribute(&mut self, until: EnumSet<Kind>) -> ParseResult<bool> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a comma or a right curlybrace,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a period or an equals, this is a
        // NODE_ATTRIBUTE.
        self.parse_identifier(
            until | TOKEN_COLON_EQUAL | TOKEN_PERIOD | EXPRESSION_TOKENS | TOKEN_COMMA,
        );

        if matches!(
            self.peek_expecting(
                TOKEN_PERIOD | TOKEN_COLON_EQUAL | TOKEN_COMMA | TOKEN_RIGHT_CURLYBRACE
            )?,
            TOKEN_COMMA | TOKEN_RIGHT_CURLYBRACE
        ) {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                found(this.next_if(TOKEN_COMMA))
            })
        } else {
            self.node_from(checkpoint, NODE_ATTRIBUTE, |this| {
                this.node_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_expecting(TOKEN_PERIOD | TOKEN_COLON_EQUAL)? == TOKEN_PERIOD {
                        this.next().unwrap();
                        this.parse_identifier(
                            until | TOKEN_COLON_EQUAL | EXPRESSION_TOKENS | TOKEN_COMMA,
                        );
                    }
                    found(())
                })?;

                this.expect(
                    TOKEN_COLON_EQUAL.into(),
                    until | EXPRESSION_TOKENS | TOKEN_COMMA,
                )?;
                this.parse_expression(until | TOKEN_COMMA)?;
                found(this.next_if(TOKEN_COMMA))
            })
        }
    }

    fn parse_lambda(&mut self, checkpoint: rowan::Checkpoint, until: EnumSet<Kind>) {
        self.node_failable_from(checkpoint, NODE_LAMBDA, |this| {
            this.node_from(checkpoint, NODE_LAMBDA_PARAMETER_IDENTIFIER, |_| {});

            this.expect(TOKEN_EQUAL_GREATER.into(), until | EXPRESSION_TOKENS)?;

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
                        | TOKEN_EQUAL_GREATER,
                )?;

                while !(until | TOKEN_RIGHT_CURLYBRACE)
                    .contains(this.peek_expecting(IDENTIFIER_TOKENS | TOKEN_RIGHT_CURLYBRACE)?)
                {
                    let Ok(Some(true)) =
                        this.parse_lambda_pattern_attribute(until | TOKEN_RIGHT_CURLYBRACE)
                    else {
                        break;
                    };
                }

                this.expect(
                    TOKEN_RIGHT_CURLYBRACE.into(),
                    until | TOKEN_EQUAL_GREATER | EXPRESSION_TOKENS,
                )
            })?;

            this.expect(TOKEN_EQUAL_GREATER.into(), until | EXPRESSION_TOKENS)?;

            this.parse_expression(until)
        });
    }

    fn parse_lambda_pattern_attribute(&mut self, until: EnumSet<Kind>) -> ParseResult<bool> {
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

            let next = this.peek_expecting(TOKEN_COMMA | TOKEN_RIGHT_CURLYBRACE)?;

            if next != TOKEN_RIGHT_CURLYBRACE {
                this.expect(TOKEN_COMMA.into(), until)?;
                found(true)
            } else {
                found(false)
            }
        })
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

                    _ => break found(()),
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
                        this.parse_interpolation()?;
                    },

                    other if other == end => {
                        this.next_direct().unwrap();
                        break Ok(Some(()));
                    },

                    _ => {
                        // Sometimes parsing interpolation leaves us unwanted tokens. It is not
                        // worth it trying to parse it correctly without a big rewrite, so just
                        // consume them.
                        this.next_direct().unwrap();
                    },
                }
            }
        });
    }

    fn parse_interpolation(&mut self) -> ParseResult {
        self.node(NODE_INTERPOLATION, |this| {
            this.expect(TOKEN_INTERPOLATION_START.into(), EnumSet::empty())?;

            this.parse_expression(TOKEN_INTERPOLATION_END.into())?;

            this.expect(TOKEN_INTERPOLATION_END.into(), EnumSet::empty())?;

            found(())
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

            if this.next_if(TOKEN_LITERAL_ELSE) {
                this.parse_expression(until)?;
            }
            Ok(Some(()))
        });
    }

    fn parse_expression_single(&mut self, until: EnumSet<Kind>) -> ParseResult {
        if self.depth >= 512 {
            let error = ParseError::RecursionLimitExceeded { at: self.offset };

            self.node(NODE_ERROR, |this| {
                this.next_direct_while(|_| true);
            });

            return deadly(error);
        }

        self.depth += 1;

        let checkpoint = self.checkpoint();

        match self.peek_expecting(EXPRESSION_TOKENS)? {
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

                if let Some(TOKEN_EQUAL_GREATER) = self.peek() {
                    self.parse_lambda(checkpoint, until)
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

            unexpected => {
                // TODO: Find a way to merge this with expect?
                self.next_while_trivia();
                let start = self.offset;

                self.node(NODE_ERROR, |this| {
                    this.next_while(|kind| !(until | EXPRESSION_TOKENS).contains(kind));
                });

                let error = ParseError::Unexpected {
                    got: Some(unexpected),
                    expected: EXPRESSION_TOKENS,
                    at: rowan::TextRange::new(start, self.offset),
                };

                let next = self.peek();

                return if next.is_some_and(|kind| EXPRESSION_TOKENS.contains(kind)) {
                    self.errors.push(error);
                    self.parse_expression_single(until)
                } else if next.is_some() {
                    self.errors.push(error);
                    recoverable()
                } else {
                    deadly(error)
                };
            },
        }

        while self.next_if(TOKEN_PERIOD) {
            self.node_failable_from(checkpoint, NODE_ATTRIBUTE_SELECT, |this| {
                this.parse_identifier(until | TOKEN_LITERAL_OR | EXPRESSION_TOKENS);

                if this.next_if(TOKEN_LITERAL_OR) {
                    this.parse_expression(until)?;
                }

                found(())
            });
        }

        // TODO: Make this an InfixOperation while handling the path-ness properly.
        if self.next_if(TOKEN_QUESTIONMARK) {
            self.node_from(checkpoint, NODE_ATTRIBUTE_CHECK, |this| {
                this.parse_identifier(until | TOKEN_PERIOD);

                while this.next_if(TOKEN_PERIOD) {
                    this.parse_identifier(until | TOKEN_PERIOD);
                }
            });
        }

        self.depth -= 1;
        found(())
    }

    fn parse_expression_application(&mut self, until: EnumSet<Kind>) -> ParseResult {
        let checkpoint = self.checkpoint();

        self.parse_expression_single(until)?;

        while self.peek().is_some_and(Kind::is_argument) {
            self.node_failable_from(checkpoint, NODE_APPLICATION, |this| {
                this.parse_expression_single(until)
            });
        }

        found(())
    }

    fn parse_expression_binding_power(
        &mut self,
        minimum_power: u16,
        until: EnumSet<Kind>,
    ) -> ParseResult {
        let checkpoint = self.checkpoint();

        if let Some(((), right_power)) = self.peek().and_then(|next| {
            node::PrefixOperator::try_from(next)
                .map(|operator| operator.binding_power())
                .ok()
        }) {
            self.node_failable(NODE_PREFIX_OPERATION, |this| {
                this.next().unwrap();
                this.parse_expression_binding_power(right_power, until)
            });
        } else {
            self.parse_expression_application(until)?;
        }

        while let Some((left_power, right_power)) = self.peek().and_then(|next| {
            node::InfixOperator::try_from(next)
                .map(|operator| operator.binding_power())
                .ok()
        }) {
            if left_power < minimum_power {
                break;
            }

            self.node_failable_from(checkpoint, NODE_INFIX_OPERATION, |this| {
                this.next().unwrap();
                this.parse_expression_binding_power(right_power, until)
            });
        }

        // TODO: Suffix operation.

        found(())
    }

    fn parse_expression(&mut self, until: EnumSet<Kind>) -> ParseResult {
        self.parse_expression_binding_power(0, until)
    }
}

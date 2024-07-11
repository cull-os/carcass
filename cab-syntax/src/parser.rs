use std::{
    fmt,
    iter::Peekable,
    panic::Location,
};

use enumset::EnumSet;
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
    TokenizerToken,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    RecursionLimitExceeded,

    Unexpected {
        got: Option<Kind>,
        expected: Option<EnumSet<Kind>>,
        at: rowan::TextRange,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecursionLimitExceeded => write!(formatter, "recursion limit exceeded"),

            Self::Unexpected {
                got: None,
                expected: Some(expected),
                at,
            } => {
                assert_eq!(at.start(), at.end());

                write!(formatter, "expected {expected:?}, reached end of file")
            },

            Self::Unexpected {
                got: Some(got),
                expected: None,
                at,
            } => {
                write!(formatter, "expected end of file, got {got:?} at {at:?}")
            },

            Self::Unexpected {
                got: Some(got),
                expected: Some(expected),
                at,
            } => {
                write!(formatter, "expected {expected:?} got {got:?} at {at:?}")
            },

            other => {
                dbg!(other);
                Ok(())
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse {
    node: rowan::GreenNode,
    errors: Vec<ParseError>,
}

impl Parse {
    pub fn syntax(self) -> RowanNode {
        RowanNode::new_root(self.node)
    }

    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    pub fn root(self) -> Root {
        Root::cast(self.syntax()).unwrap()
    }

    pub fn result(self) -> Result<Root, Vec<ParseError>> {
        if self.errors.is_empty() {
            Ok(self.root())
        } else {
            Err(self.errors)
        }
    }
}

pub fn parse(input: &str) -> Parse {
    let mut parser = Parser::new(tokenize(input));

    parser
        .node(NODE_ROOT, |this| {
            let checkpoint = this.checkpoint();

            // Reached an unrecoverable error.
            if let Err(error) = this.parse_expression() {
                log::trace!("unrecoverable error encountered: {error:?}");

                this.node_from(checkpoint, NODE_ERROR, |_| Ok(())).unwrap();

                this.errors.push(error);
            }

            if let Some(got) = this.peek_nontrivia() {
                log::trace!("leftovers encountered: {got:?}");

                let start = this.offset;

                this.node(NODE_ERROR, |this| {
                    this.next_while(|_| true);
                    Ok(())
                })
                .unwrap();

                this.errors.push(ParseError::Unexpected {
                    got: Some(got),
                    expected: None,
                    at: rowan::TextRange::new(start, this.offset),
                });
            }
            Ok(())
        })
        .unwrap();

    Parse {
        node: parser.builder.finish(),
        errors: parser.errors,
    }
}

struct Parser<'a, I: Iterator<Item = TokenizerToken<'a>>> {
    builder: rowan::GreenNodeBuilder<'a>,

    tokens: Peekable<I>,
    errors: Vec<ParseError>,

    offset: rowan::TextSize,
    depth: u32,
}

impl<'a, I: Iterator<Item = TokenizerToken<'a>>> Parser<'a, I> {
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
        self.tokens.peek().map(|token| token.0)
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
                at: rowan::TextRange::new(self.offset, self.offset),
            }
        })
    }

    fn next(&mut self) -> Result<Kind, ParseError> {
        self.tokens
            .next()
            .map(|TokenizerToken(kind, slice)| {
                self.offset += rowan::TextSize::of(slice);
                self.builder.token(Language::kind_to_raw(kind), slice);
                kind
            })
            .ok_or_else(|| {
                ParseError::Unexpected {
                    got: None,
                    expected: None,
                    at: rowan::TextRange::new(self.offset, self.offset),
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
        self.expect_until(expected, EnumSet::empty())
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
                    Ok(())
                })
                .unwrap();

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
                    at: rowan::TextRange::new(self.offset, self.offset),
                })
            },
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    #[track_caller]
    fn node(
        &mut self,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), ParseError>,
    ) -> Result<(), ParseError> {
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
    fn node_failable(
        &mut self,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), ParseError>,
    ) {
        let checkpoint = self.checkpoint();

        if let Err(error) = self.node(kind, closure) {
            self.errors.push(error);
            self.node_from(checkpoint, NODE_ERROR, |_| Ok(())).unwrap();
        }
    }

    #[track_caller]
    fn node_from(
        &mut self,
        checkpoint: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), ParseError>,
    ) -> Result<(), ParseError> {
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
            self.node_from(checkpoint, NODE_ERROR, |_| Ok(())).unwrap();
        }
    }

    fn parse_stringish_inner<const END: Kind>(&mut self) -> Result<(), ParseError> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let checkpoint = self.checkpoint();

            let current = self.expect(TOKEN_CONTENT | TOKEN_INTERPOLATION_START | END)?;

            if current == TOKEN_INTERPOLATION_START {
                self.node_from(checkpoint, NODE_INTERPOLATION, |this| {
                    this.parse_expression_until(TOKEN_INTERPOLATION_END.into())?;
                    this.expect(TOKEN_INTERPOLATION_END.into())?;
                    Ok(())
                })?;
            } else if current == END {
                break;
            }
        }

        Ok(())
    }

    fn parse_identifier(&mut self) -> Result<(), ParseError> {
        self.node(NODE_IDENTIFIER, |this| {
            // If it is a normal identifier, we don't do anything
            // else as it only has a single token, and .expect() consumes it.
            if this.expect(TOKEN_IDENTIFIER | TOKEN_IDENTIFIER_START)? == TOKEN_IDENTIFIER_START {
                this.parse_stringish_inner::<{ TOKEN_IDENTIFIER_END }>()?;
            }
            Ok(())
        })?;

        Ok(())
    }

    fn parse_attribute(&mut self) -> Result<(), ParseError> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a semicolon,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a period or
        // an equals, this is a NODE_ATTRIBUTE.
        self.parse_identifier()?;

        if self.peek_nontrivia_expecting(TOKEN_SEMICOLON | TOKEN_PERIOD | TOKEN_EQUAL)?
            == TOKEN_SEMICOLON
        {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                this.next().unwrap();
                Ok(())
            })
            .unwrap();
        } else {
            self.node_failable_from(checkpoint, NODE_ATTRIBUTE_ENTRY, |this| {
                this.node_failable_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_nontrivia_expecting(TOKEN_PERIOD | TOKEN_EQUAL)? != TOKEN_EQUAL
                    {
                        this.expect_until(TOKEN_PERIOD.into(), TOKEN_EQUAL.into())?;
                        this.parse_identifier()?;
                    }
                    Ok(())
                });

                this.expect_until(TOKEN_EQUAL.into(), TOKEN_SEMICOLON | TOKEN_RIGHT_CURLYBRACE)?;
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
            this.parse_expression()?;
            this.expect(TOKEN_INTERPOLATION_END.into())?;
            Ok(())
        })
    }

    fn parse_expression(&mut self) -> Result<(), ParseError> {
        self.parse_expression_until(EnumSet::empty())
    }

    fn parse_expression_until(&mut self, until: EnumSet<Kind>) -> Result<(), ParseError> {
        if self.depth >= 512 {
            self.node(NODE_ERROR, |this| {
                this.next_while(|_| true);
                Ok(())
            })
            .unwrap();

            return Err(ParseError::RecursionLimitExceeded);
        }

        self.depth += 1;

        let checkpoint = self.checkpoint();

        match self.expect_until(
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
                | TOKEN_LITERAL_IF,
            until,
        )? {
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
                        let peek = this.peek_nontrivia_expecting(TOKEN_RIGHT_BRACKET.into())?;
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

            Some(TOKEN_LEFT_CURLYBRACE) => {
                self.node_failable_from(checkpoint, NODE_ATTRIBUTE_SET, |this| {
                    while this.peek_nontrivia_expecting(TOKEN_RIGHT_CURLYBRACE.into())?
                        != TOKEN_RIGHT_CURLYBRACE
                    {
                        this.parse_attribute()?;
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

            Some(TOKEN_IDENTIFIER) => {
                self.node_from(checkpoint, NODE_IDENTIFIER, |_| Ok(()))
                    .unwrap();
            },

            Some(TOKEN_IDENTIFIER_START) => {
                self.node_failable_from(checkpoint, NODE_IDENTIFIER, |this| {
                    this.parse_stringish_inner::<{ TOKEN_IDENTIFIER_END }>()
                });
            },

            Some(TOKEN_STRING_START) => {
                self.node_failable_from(checkpoint, NODE_STRING, |this| {
                    this.parse_stringish_inner::<{ TOKEN_STRING_END }>()
                });
            },

            Some(TOKEN_ISLAND_START) => {
                self.node_failable_from(checkpoint, NODE_ISLAND, |this| {
                    this.parse_stringish_inner::<{ TOKEN_ISLAND_END }>()
                });
            },

            Some(TOKEN_INTEGER | TOKEN_FLOAT) => {
                self.node_from(checkpoint, NODE_NUMBER, |_| Ok(())).unwrap();
            },

            Some(TOKEN_LITERAL_IF) => {
                self.node_failable_from(checkpoint, NODE_IF_ELSE, |this| {
                    this.parse_expression_until(until | TOKEN_LITERAL_THEN)?;
                    this.expect(TOKEN_LITERAL_THEN.into())?;
                    this.parse_expression_until(until | TOKEN_LITERAL_ELSE)?;

                    if this.peek_nontrivia() == Some(TOKEN_LITERAL_ELSE) {
                        this.next().unwrap();
                        this.parse_expression_until(until)?;
                    }
                    Ok(())
                });
            },

            None => {
                self.node_from(checkpoint, NODE_ERROR, |_| Ok(())).unwrap();
            },

            _ => unreachable!(),
        }

        self.depth -= 1;
        Ok(())
    }
}

use std::{
    iter::Peekable,
    panic::Location,
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
    TokenizerToken,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Unexpected {
        got: Option<Kind>,
        expected: Option<&'static [Kind]>,
        at: rowan::TextRange,
    },

    RecursionLimitExceeded,
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

            if let Ok(got) = this.peek_nontrivia() {
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

    fn peek(&mut self) -> Result<Kind, ParseError> {
        self.tokens.peek().map(|token| token.0).ok_or_else(|| {
            ParseError::Unexpected {
                got: None,
                expected: None,
                at: rowan::TextRange::new(self.offset, self.offset),
            }
        })
    }

    fn peek_nontrivia(&mut self) -> Result<Kind, ParseError> {
        self.next_while(Kind::is_trivia);
        self.peek()
    }

    fn peek_nontrivia_expecting(&mut self, expected: &'static [Kind]) -> Result<Kind, ParseError> {
        self.peek_nontrivia().map_err(|error| {
            let ParseError::Unexpected { got, at, .. } = error else {
                unreachable!()
            };

            ParseError::Unexpected {
                got,
                expected: Some(expected),
                at,
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

    fn expect(&mut self, expected: &'static [Kind]) -> Result<Kind, ParseError> {
        let checkpoint = self.checkpoint();

        match self.next_nontrivia() {
            Ok(got) if expected.contains(&got) => Ok(got),

            Ok(unexpected) => {
                let start = self.offset;

                self.node_from(checkpoint, NODE_ERROR, |this| {
                    this.next_while(|kind| !expected.contains(&kind));
                    Ok(())
                })?;

                let error = ParseError::Unexpected {
                    got: Some(unexpected),
                    expected: Some(expected),
                    at: rowan::TextRange::new(start, self.offset),
                };

                if let Ok(kind) = self.next_nontrivia() {
                    self.errors.push(error);
                    Ok(kind)
                } else {
                    Err(error)
                }
            },

            Err(_) => {
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
    fn node_from(
        &mut self,
        at: rowan::Checkpoint,
        kind: Kind,
        closure: impl FnOnce(&mut Self) -> Result<(), ParseError>,
    ) -> Result<(), ParseError> {
        log::trace!(
            "starting node {kind:?} at {at:?} in {location}",
            location = Location::caller()
        );
        self.builder.start_node_at(at, Language::kind_to_raw(kind));

        let result = closure(self);

        log::trace!("ending node at {location}", location = Location::caller());
        self.builder.finish_node();
        result
    }

    fn parse_stringish_inner<const END: Kind>(&mut self) -> Result<(), ParseError> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let checkpoint = self.checkpoint();

            let current = self.expect(&[TOKEN_CONTENT, TOKEN_INTERPOLATION_START, END])?;

            if current == TOKEN_INTERPOLATION_START {
                self.node_from(checkpoint, NODE_INTERPOLATION, |this| {
                    this.parse_expression()?;
                    this.expect(&[TOKEN_INTERPOLATION_END])?;
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
            if this.expect(&[TOKEN_IDENTIFIER, TOKEN_IDENTIFIER_START])? == TOKEN_IDENTIFIER_START {
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

        if self.peek_nontrivia_expecting(&[TOKEN_SEMICOLON, TOKEN_PERIOD, TOKEN_EQUAL])?
            == TOKEN_SEMICOLON
        {
            self.node_from(checkpoint, NODE_ATTRIBUTE_INHERIT, |this| {
                this.next().unwrap();
                Ok(())
            })?;
        } else {
            self.node_from(checkpoint, NODE_ATTRIBUTE_ENTRY, |this| {
                this.node_from(checkpoint, NODE_ATTRIBUTE_PATH, |this| {
                    while this.peek_nontrivia_expecting(&[TOKEN_PERIOD, TOKEN_EQUAL])?
                        != TOKEN_EQUAL
                    {
                        this.expect(&[TOKEN_PERIOD])?;
                        this.parse_identifier()?;
                    }
                    Ok(())
                })?;

                this.expect(&[TOKEN_EQUAL])?;
                this.parse_expression()?;
                this.expect(&[TOKEN_SEMICOLON])?;
                Ok(())
            })?;
        }

        Ok(())
    }

    fn parse_expression(&mut self) -> Result<(), ParseError> {
        if self.depth >= 512 {
            self.node(NODE_ERROR, |this| {
                this.next_while(|_| true);
                Ok(())
            })?;

            return Err(ParseError::RecursionLimitExceeded);
        }

        let checkpoint = self.checkpoint();

        match self.expect(&[
            TOKEN_LEFT_PARENTHESIS,
            TOKEN_LEFT_BRACKET,
            TOKEN_LEFT_CURLYBRACE,
            TOKEN_LITERAL_IF,
            TOKEN_IDENTIFIER,
            TOKEN_IDENTIFIER_START,
        ])? {
            TOKEN_LEFT_PARENTHESIS => {
                self.node_from(checkpoint, NODE_PARENTHESIS, |this| {
                    this.parse_expression()?;
                    this.expect(&[TOKEN_RIGHT_PARENTHESIS])?;
                    Ok(())
                })?
            },

            TOKEN_LEFT_BRACKET => {
                self.node_from(checkpoint, NODE_LIST, |this| {
                    while this.peek_nontrivia_expecting(&[TOKEN_RIGHT_BRACKET])?
                        != TOKEN_RIGHT_BRACKET
                    {
                        // TODO: Seperate expression parsing logic into two functions
                        // to not parse multiple expressions next to eachother as an
                        // application chain.
                        this.parse_expression()?;
                    }

                    this.expect(&[TOKEN_RIGHT_BRACKET])?;
                    Ok(())
                })?;
            },

            TOKEN_LEFT_CURLYBRACE => {
                self.node_from(checkpoint, NODE_ATTRIBUTE_SET, |this| {
                    while this.peek_nontrivia_expecting(&[TOKEN_RIGHT_CURLYBRACE])?
                        != TOKEN_RIGHT_CURLYBRACE
                    {
                        this.parse_attribute()?;
                    }

                    this.expect(&[TOKEN_RIGHT_CURLYBRACE])?;
                    Ok(())
                })?;
            },

            TOKEN_LITERAL_IF => {
                self.node_from(checkpoint, NODE_IF_ELSE, |this| {
                    this.parse_expression()?;
                    this.expect(&[TOKEN_LITERAL_THEN])?;
                    this.parse_expression()?;

                    if this.peek_nontrivia() == Ok(TOKEN_LITERAL_ELSE) {
                        this.next_nontrivia().expect("peek returned Some");
                        this.parse_expression()?;
                    }
                    Ok(())
                })?;
            },

            TOKEN_IDENTIFIER => {
                self.node_from(checkpoint, NODE_IDENTIFIER, |_| Ok(()))?;
            },

            TOKEN_IDENTIFIER_START => {
                self.node_from(checkpoint, NODE_IDENTIFIER, |this| {
                    this.parse_stringish_inner::<{ TOKEN_IDENTIFIER_END }>()
                })?;
            },

            TOKEN_PLUS | TOKEN_MINUS | TOKEN_LITERAL_NOT => {
                self.node_from(checkpoint, NODE_PREFIX_OPERATION, |this| {
                    this.parse_expression()
                })?;
            },

            TOKEN_PATH => {
                self.node_from(checkpoint, NODE_PATH, |this| {
                    loop {
                        let checkpoint = this.checkpoint();
                        let current = this.next();

                        if current == Ok(TOKEN_INTERPOLATION_START) {
                            this.node_from(checkpoint, NODE_INTERPOLATION, |this| {
                                this.parse_expression()?;
                                this.expect(&[TOKEN_INTERPOLATION_END])?;
                                Ok(())
                            })?;
                        } else if current != Ok(TOKEN_PATH) {
                            break;
                        }
                    }
                    Ok(())
                })?;
            },

            _ => unreachable!(),
        }

        Ok(())
    }
}

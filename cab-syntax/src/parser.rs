#![allow(unused)]

use std::{
    collections::VecDeque,
    iter::Peekable,
};

use rowan::{
    ast::AstNode as _,
    Language as _,
};

use crate::{
    ast::node::Expression,
    syntax::{
        self,
        Kind::*,
    },
    tokenizer::Token,
    Language,
    Tokenizer,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Unexpected {
        got: Option<syntax::Kind>,
        expected: Option<&'static [syntax::Kind]>,
        at: rowan::TextRange,
    },

    RecursionLimitExceeded,
}

#[derive(Debug)]
struct Parser<'a, I: Iterator<Item = Token<'a>>> {
    builder: rowan::GreenNodeBuilder<'static>,

    tokens: Peekable<I>,
    errors: Vec<ParseError>,

    offset: rowan::TextSize,

    depth: u32,
}

impl<'a, I: Iterator<Item = Token<'a>>> Parser<'a, I> {
    fn new(tokens: I) -> Self {
        Self {
            builder: rowan::GreenNodeBuilder::new(),

            tokens: tokens.peekable(),
            errors: Vec::new(),

            offset: 0.into(),

            depth: 0,
        }
    }

    #[must_use]
    fn peek(&mut self) -> Option<syntax::Kind> {
        self.tokens.peek().map(|token| token.0)
    }

    #[must_use]
    fn peek_nontrivia(&mut self) -> Option<syntax::Kind> {
        self.next_while(syntax::Kind::is_trivia);
        self.peek()
    }

    #[must_use]
    fn next(&mut self) -> Option<syntax::Kind> {
        self.tokens.next().map(|Token(kind, slice)| {
            self.offset += rowan::TextSize::of(slice);
            self.builder.token(Language::kind_to_raw(kind), slice);
            kind
        })
    }

    #[must_use]
    fn next_nontrivia(&mut self) -> Option<syntax::Kind> {
        self.next_while_trivia();
        self.next()
    }

    #[must_use]
    fn next_nontrivia_if(
        &mut self,
        predicate: impl Fn(syntax::Kind) -> bool,
    ) -> Option<syntax::Kind> {
        self.next_while_trivia();
        self.peek()
            .map_or(false, predicate)
            .then(|| self.next())
            .flatten()
    }

    fn next_while(&mut self, predicate: impl Fn(syntax::Kind) -> bool) {
        while self.peek().map_or(false, &predicate) {
            self.next();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_while(syntax::Kind::is_trivia)
    }

    #[must_use]
    fn expect(&mut self, expected: &'static [syntax::Kind]) -> Option<syntax::Kind> {
        match self.next_nontrivia() {
            Some(got) if expected.contains(&got) => Some(got),
            unexpected => {
                self.errors.push(ParseError::Unexpected {
                    got: unexpected,
                    expected: Some(expected),
                    at: rowan::TextRange::new(self.offset, self.offset),
                });

                self.node_start(NODE_ERROR);
                self.next_while(|kind| !expected.contains(&kind));
                self.node_end();

                self.next_nontrivia()
            },
        }
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    fn node_start(&mut self, kind: syntax::Kind) {
        self.builder.start_node(Language::kind_to_raw(kind));
    }

    fn node_start_at(&mut self, at: rowan::Checkpoint, kind: syntax::Kind) {
        self.builder.start_node_at(at, Language::kind_to_raw(kind))
    }

    fn node_end(&mut self) {
        self.builder.finish_node();
    }

    fn parse_stringish(&mut self, end: syntax::Kind) -> Option<()> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let peek = self.peek_nontrivia()?;

            if peek == TOKEN_IDENTIFIER_START {
                self.node_start(NODE_INTERPOLATION);
                self.next().expect("peek is Some");
                self.parse_expression()?;
                self.expect(&[TOKEN_INTERPOLATION_END])?;
                self.node_end();
            } else {
                // debug_assert!(peek == end || peek == TOKEN_CONTENT);
                // dbg!(peek);
                self.next().expect("peek is Some");
            }

            if peek == end {
                break;
            }
        }

        Some(())
    }

    #[must_use]
    fn parse_identifier(&mut self) -> Option<()> {
        self.node_start(NODE_IDENTIFIER);

        match self.expect(&[TOKEN_IDENTIFIER, TOKEN_IDENTIFIER_START])? {
            TOKEN_IDENTIFIER => {
                self.next().unwrap();
            },
            TOKEN_IDENTIFIER_START => {
                self.parse_stringish(TOKEN_IDENTIFIER_END);
            },
            _ => unreachable!(),
        }

        self.node_end();

        Some(())
    }

    #[must_use]
    fn parse_attribute(&mut self) -> Option<()> {
        let checkpoint = self.checkpoint();

        // First identifier down. If the next token is a semicolon,
        // this is a NODE_ATTRIBUTE_INHERIT. If it is a colon or
        // an equals, this is a NODE_ATTRIBUTE.
        self.parse_identifier()?;

        if self.peek_nontrivia()? == TOKEN_SEMICOLON {
            self.node_start_at(checkpoint, NODE_ATTRIBUTE_INHERIT);
            self.next().expect("peek returned Some");
            self.node_end();
        } else {
            self.node_start_at(checkpoint, NODE_ATTRIBUTE);
            self.node_start_at(checkpoint, NODE_ATTRIBUTE_PATH);
            while self.peek_nontrivia() != Some(TOKEN_EQUAL) {
                self.expect(&[TOKEN_PERIOD])?;
                self.parse_identifier()?;
            }
            self.node_end();

            self.expect(&[TOKEN_EQUAL])?;
            self.parse_expression()?;
            self.expect(&[TOKEN_SEMICOLON])?;
            self.node_end();
        }

        Some(())
    }

    #[must_use]
    fn parse_expression(&mut self) -> Option<()> {
        if self.depth >= 512 {
            self.errors.push(ParseError::RecursionLimitExceeded);

            self.node_start(NODE_ERROR);
            self.next_while(|_| true);
            self.node_end();

            return None;
        }

        match self.peek_nontrivia() {
            Some(TOKEN_LEFT_PARENTHESIS) => {
                self.node_start(NODE_PARENTHESIS);
                self.next().expect("peek returned Some");

                self.parse_expression()?;

                self.expect(&[TOKEN_RIGHT_PARENTHESIS])?;
                self.node_end();
            },

            Some(TOKEN_LEFT_BRACKET) => {
                self.node_start(NODE_LIST);
                self.next().expect("peek returned Some");

                while self.peek_nontrivia() != Some(TOKEN_RIGHT_BRACKET) {
                    // TODO: Seperate expression parsing logic into two functions
                    // to not parse multiple expressions next to eachother as an
                    // application chain.
                    self.parse_expression()?;
                }

                self.expect(&[TOKEN_RIGHT_BRACKET])?;
                self.node_end();
            },

            Some(TOKEN_LEFT_CURLYBRACE) => {
                self.node_start(NODE_ATTRIBUTE_SET);
                self.next().expect("peek returned Some");

                while self.peek_nontrivia() != Some(TOKEN_RIGHT_CURLYBRACE) {
                    self.parse_attribute()?;
                }

                self.expect(&[TOKEN_RIGHT_CURLYBRACE])?;
                self.node_end();
            },

            Some(TOKEN_LITERAL_IF) => {
                self.next().expect("peek returned Some");
                self.parse_expression()?;
                self.expect(&[TOKEN_LITERAL_THEN])?;
                self.parse_expression()?;

                if self.peek() == Some(TOKEN_LITERAL_ELSE) {
                    self.next_nontrivia().expect("peek returned Some");
                    self.parse_expression()?;
                }
            },

            unexpected => {
                self.errors.push(ParseError::Unexpected {
                    got: unexpected,
                    expected: None,
                    at: rowan::TextRange::new(self.offset, self.offset),
                })
            },
        }

        Some(())
    }
}

impl Expression {
    pub fn parse(input: &str) -> Result<Expression, Vec<ParseError>> {
        let mut tokenizer = Tokenizer::new(input);
        let mut parser = Parser::new(tokenizer);

        let _ = parser.parse_expression();
        let node = parser.builder.finish();

        if parser.errors.is_empty() {
            Expression::cast(syntax::Node::new_root(node)).ok_or_else(Vec::new)
        } else {
            Err(parser.errors)
        }
    }
}

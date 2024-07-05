#![allow(unused)]

use std::{
    collections::VecDeque,
    iter::Peekable,
    marker::PhantomData,
};

use rowan::{
    ast::AstNode as _,
    Language as _,
};

use crate::{
    node::Expression,
    tokenize,
    Kind,
    Kind::*,
    Language,
    Node,
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

impl<T> Parse<T> {
    pub fn syntax(self) -> RowanNode {
        RowanNode::new_root(self.node)
    }
}

impl<T: Node> Parse<T> {
    pub fn tree(self) -> T {
        T::cast(self.syntax()).unwrap()
    }

    pub fn result(self) -> Result<T, Vec<ParseError>> {
        if self.errors.is_empty() {
            Ok(self.tree())
        } else {
            Err(self.errors)
        }
    }
}

pub fn parse<T>(input: &str) -> Parse<T> {
    let mut parser = Parser::new(tokenize(input));

    let _ = parser.parse_expression();
    let node = parser.builder.finish();

    Parse {
        node,
        errors: parser.errors,

        r#type: PhantomData,
    }
}

#[derive(Debug)]
struct Parser<'a, I: Iterator<Item = TokenizerToken<'a>>> {
    builder: rowan::GreenNodeBuilder<'static>,

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

    #[must_use]
    fn peek(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|token| token.0)
    }

    #[must_use]
    fn peek_nontrivia(&mut self) -> Option<Kind> {
        self.next_while(Kind::is_trivia);
        self.peek()
    }

    #[must_use]
    fn next(&mut self) -> Option<Kind> {
        self.tokens.next().map(|TokenizerToken(kind, slice)| {
            self.offset += rowan::TextSize::of(slice);
            self.builder.token(Language::kind_to_raw(kind), slice);
            kind
        })
    }

    #[must_use]
    fn next_nontrivia(&mut self) -> Option<Kind> {
        self.next_while_trivia();
        self.next()
    }

    #[must_use]
    fn next_nontrivia_if(&mut self, predicate: impl Fn(Kind) -> bool) -> Option<Kind> {
        self.next_while_trivia();
        self.peek()
            .map_or(false, predicate)
            .then(|| self.next())
            .flatten()
    }

    fn next_while(&mut self, predicate: impl Fn(Kind) -> bool) {
        while self.peek().map_or(false, &predicate) {
            self.next();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_while(Kind::is_trivia)
    }

    #[must_use]
    fn expect(&mut self, expected: &'static [Kind]) -> Option<Kind> {
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

    fn node_start(&mut self, kind: Kind) {
        self.builder.start_node(Language::kind_to_raw(kind));
    }

    fn node_start_at(&mut self, at: rowan::Checkpoint, kind: Kind) {
        self.builder.start_node_at(at, Language::kind_to_raw(kind))
    }

    fn node_end(&mut self) {
        self.builder.finish_node();
    }

    fn parse_stringish(&mut self, end: Kind) -> Option<()> {
        // Assuming that the start quote has already been consumed
        // and the node is closed outside of this function.

        loop {
            let peek = self.peek_nontrivia()?;

            if peek == TOKEN_INTERPOLATION_START {
                self.node_start(NODE_INTERPOLATION);
                self.next().expect("peek is Some");
                self.parse_expression()?;
                self.expect(&[TOKEN_INTERPOLATION_END])?;
                self.node_end();
            } else {
                debug_assert!(peek == end || peek == TOKEN_CONTENT);
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

        // If it is a normal identifier, we don't do anything
        // else as it only has a single token, and .expect() consumes it.
        if self.expect(&[TOKEN_IDENTIFIER, TOKEN_IDENTIFIER_START])? == TOKEN_IDENTIFIER_START {
            self.parse_stringish(TOKEN_IDENTIFIER_END);
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
        dbg!(self.peek_nontrivia());

        if self.peek_nontrivia()? == TOKEN_SEMICOLON {
            dbg!("next is a semicolon");
            self.node_start_at(checkpoint, NODE_ATTRIBUTE_INHERIT);
            self.next().expect("peek returned Some");
            self.node_end();
        } else {
            dbg!("next is not a semicolon");
            self.node_start_at(checkpoint, NODE_ATTRIBUTE);
            self.node_start_at(checkpoint, NODE_ATTRIBUTE_PATH);
            while self.peek_nontrivia()? != TOKEN_EQUAL {
                dbg!(self.peek_nontrivia());
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

                while self.peek_nontrivia()? != TOKEN_RIGHT_BRACKET {
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

                while self.peek_nontrivia()? != TOKEN_RIGHT_CURLYBRACE {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parse<T> {
    node: rowan::GreenNode,
    errors: Vec<ParseError>,

    r#type: PhantomData<T>,
}

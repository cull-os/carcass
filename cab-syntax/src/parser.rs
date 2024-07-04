#![allow(unused)]

use std::collections::VecDeque;

use rowan::Language as _;

use crate::{
    syntax::{
        self,
        Kind::*,
    },
    tokenizer::Token,
    Language,
};

pub enum ParseError {
    Unexpected {
        got: Option<syntax::Kind>,
        expected: Option<&'static [syntax::Kind]>,
        at: rowan::TextRange,
    },

    RecursionLimitExceeded,
}

struct Parser<'a, I: Iterator<Item = Token<'a>>> {
    builder: rowan::GreenNodeBuilder<'static>,

    tokens: I,
    errors: Vec<ParseError>,

    buffer: VecDeque<I::Item>,
    trivia_buffer: Vec<I::Item>,

    position: rowan::TextSize,

    depth: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chase {
    Found(syntax::Kind),
    Absent(Option<syntax::Kind>),
}

use Chase::*;

macro_rules! chase {
    ($self:ident, $pattern:pat) => {{
        let peek = $self.peek_nontrivial_kind();

        if matches!(peek, Some($pattern)) {
            Found(peek.unwrap())
        } else {
            Absent(peek)
        }
    }};
}

macro_rules! pattern_to_list {
    ($($variant:ident)|+) => {
        [$($variant),*]
    };
}

macro_rules! expect {
    ($self:ident, $($variant:ident)|+) => {
        match chase!($self, $($variant)|*) {
            Absent(got) => {
                let start = $self.error_node_start();

                $self.bump();
                $self.bump_while(|token| !matches!(token.0, $($variant)|*));

                let end = $self.error_node_end();

                $self.errors.push(ParseError::Unexpected {
                    got,
                    expected: Some(&pattern_to_list!($($variant)|*)),
                    at: rowan::TextRange::new(start, end),
                });

                None
            },

            Found(found) => {
                $self.bump();
                Some(found)
            },
        }
    };
}

impl<'a, I: Iterator<Item = Token<'a>>> Parser<'a, I> {
    fn new(tokens: I) -> Self {
        Self {
            builder: rowan::GreenNodeBuilder::new(),

            tokens,
            errors: Vec::new(),

            buffer: VecDeque::new(),
            trivia_buffer: Vec::new(),

            position: 0.into(),

            depth: 0,
        }
    }

    fn next(&mut self) -> Option<Token<'a>> {
        self.buffer.pop_front().or_else(|| self.tokens.next())
    }

    fn peek(&mut self) -> Option<&Token<'a>> {
        if self.buffer.is_empty() {
            if let Some(token) = self.tokens.next() {
                self.buffer.push_back(token);
            }
        }

        self.buffer.front()
    }

    fn peek_nontrivial(&mut self) -> Option<&Token<'a>> {
        self.bump_while_trivia();
        self.peek()
    }

    fn peek_nontrivial_kind(&mut self) -> Option<syntax::Kind> {
        self.peek_nontrivial().map(|token| token.0)
    }

    fn bump(&mut self) {
        let Some(Token(kind, slice)) = self.next() else {
            self.errors.push(ParseError::Unexpected {
                got: None,
                expected: None,
                at: rowan::TextRange::new(self.position, self.position),
            });
            return;
        };

        self.position += rowan::TextSize::of(slice);
        self.builder.token(Language::kind_to_raw(kind), slice);
    }

    fn bump_while(&mut self, predicate: fn(&Token<'a>) -> bool) {
        while self.peek().map_or(false, predicate) {
            self.bump();
        }
    }

    fn bump_while_trivia(&mut self) {
        self.bump_while(|token| token.0.is_trivia());
    }

    fn checkpoint(&mut self) -> rowan::Checkpoint {
        self.bump_while_trivia();
        self.builder.checkpoint()
    }

    fn node_start(&mut self, kind: syntax::Kind) {
        self.bump_while_trivia();
        self.builder.start_node(Language::kind_to_raw(kind));
    }

    fn node_end(&mut self) {
        self.bump_while_trivia();
        self.builder.finish_node();
    }

    fn error_node_start(&mut self) -> rowan::TextSize {
        self.node_start(NODE_ERROR);
        self.position
    }

    fn error_node_end(&mut self) -> rowan::TextSize {
        self.node_end();
        self.position
    }

    fn parse_expression(&mut self) {
        if self.depth >= 1024 {
            self.errors.push(ParseError::RecursionLimitExceeded);

            self.error_node_start();
            self.bump_while(|_| true);
            self.error_node_end();

            return;
        }

        self.depth += 1;

        match self.peek_nontrivial_kind() {
            Some(start @ (TOKEN_IDENTIFIER_START | TOKEN_STRING_START | TOKEN_ISLAND_START)) => {
                let end = match start {
                    TOKEN_IDENTIFIER_START => TOKEN_IDENTIFIER_END,
                    TOKEN_STRING_START => TOKEN_STRING_END,
                    TOKEN_ISLAND_START => TOKEN_ISLAND_END,
                    _ => unreachable!(),
                };

                todo!();
            },

            Some(TOKEN_LITERAL_IF) => {
                self.node_start(NODE_IF_ELSE);
                self.bump();
                self.parse_expression();
                expect!(self, TOKEN_LITERAL_THEN);
                self.parse_expression();

                if expect!(self, TOKEN_LITERAL_ELSE).is_some() {
                    self.parse_expression();
                }

                self.node_end();
            },

            None => {
                self.errors.push(ParseError::Unexpected {
                    got: None,
                    expected: None,
                    at: rowan::TextRange::new(self.position, self.position),
                })
            },
        }

        self.depth -= 1;
    }
}

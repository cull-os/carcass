use std::ptr;

use crate::SyntaxKind::{
    self,
    *,
};

pub struct Token<'a>(pub SyntaxKind, pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerContext {
    InsideInterpolation { brackets: u32 },
}

#[derive(Debug, Clone)]
struct TokenizerState<'a> {
    input: &'a str,
    offset: usize,
}

impl PartialEq for TokenizerState<'_> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.input, other.input) && self.offset == other.offset
    }
}

impl Eq for TokenizerState<'_> {}

pub struct Tokenizer<'a> {
    context: Vec<TokenizerContext>,
    state: TokenizerState<'a>,
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let start_state = self.state.clone();

        self.next_syntax_kind()
            .map(|syntax_kind| Token(syntax_kind, self.consumed_since(start_state)))
    }
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            context: Vec::new(),
            state: TokenizerState { input, offset: 0 },
        }
    }

    fn remaining(&self) -> &str {
        &self.state.input[self.state.offset..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn consume_while<F: FnMut(char) -> bool>(&mut self, mut predicate: F) -> usize {
        let length: usize = self
            .remaining()
            .chars()
            .take_while(|&c| predicate(c))
            .map(char::len_utf8)
            .sum();

        self.state.offset += length;
        length
    }

    fn consume_char(&mut self, pattern: char) -> bool {
        let starts_with = Some(pattern) == self.peek();

        if starts_with {
            self.state.offset += pattern.len_utf8();
        }

        starts_with
    }

    fn consume_pattern(&mut self, pattern: &str) -> bool {
        let starts_with = self.remaining().starts_with(pattern);

        if starts_with {
            self.state.offset += pattern.len();
        }

        starts_with
    }

    fn consumed_since<'b>(&self, past: TokenizerState<'b>) -> &'b str {
        &past.input[past.offset..self.state.offset]
    }

    fn context_push(&mut self, context: TokenizerContext) {
        self.context.push(context)
    }

    fn context_pop(&mut self, context: TokenizerContext) {
        debug_assert_eq!(self.context.last(), Some(&context));
        self.context.pop();
    }

    fn next_char(&mut self) -> Option<char> {
        let next = self.peek()?;
        self.state.offset += next.len_utf8();
        Some(next)
    }

    fn next_syntax_kind(&mut self) -> Option<SyntaxKind> {
        let start_state = self.state.clone();

        if self.consume_while(char::is_whitespace) > 0 {
            return Some(TOKEN_WHITESPACE);
        }

        if self.peek() == Some('#') {
            self.consume_while(|c| c != '\r' && c != '\n');
            return Some(TOKEN_COMMENT);
        }

        Some(match self.next_char()? {
            '(' => TOKEN_LEFT_PARENTHESIS,
            ')' => TOKEN_RIGHT_PARENTHESIS,

            '+' if self.consume_char('+') => TOKEN_PLUS_PLUS,
            '[' => TOKEN_LEFT_BRACKET,
            ']' => TOKEN_RIGHT_BRACKET,

            '=' if self.consume_char('>') => TOKEN_EQUAL_MORE,
            '/' if self.consume_char('/') => TOKEN_SLASH_SLASH,
            '{' => {
                if let Some(TokenizerContext::InsideInterpolation { brackets }) =
                    self.context.last_mut()
                {
                    *brackets += 1
                }

                TOKEN_LEFT_CURLYBRACE
            },
            '}' => {
                if let Some(TokenizerContext::InsideInterpolation { brackets }) =
                    self.context.last_mut()
                {
                    match brackets.checked_sub(1) {
                        Some(new) => *brackets = new,
                        None => {
                            self.context_pop(TokenizerContext::InsideInterpolation { brackets: 0 });
                            return Some(TOKEN_INTERPOLATION_END);
                        },
                    }
                }

                TOKEN_RIGHT_CURLYBRACE
            },

            // if then else
            '=' if self.consume_char('=') => TOKEN_EQUAL_EQUAL,
            '=' => TOKEN_EQUAL,
            '!' if self.consume_char('=') => TOKEN_EXCLAMATION_EQUAL,
            '<' if self.consume_char('=') => TOKEN_LESS_EQUAL,
            '<' => TOKEN_LESS,
            '>' if self.consume_char('=') => TOKEN_MORE_EQUAL,
            '>' => TOKEN_MORE,
            '-' if self.consume_char('>') => TOKEN_MINUS_GREATER,

            // and or not
            '.' => TOKEN_PERIOD,
            '@' => TOKEN_AT,
            '?' => TOKEN_QUESTIONMARK,
            ':' => TOKEN_COLON,
            ';' => TOKEN_SEMICOLON,

            '+' => TOKEN_PLUS,
            '-' => TOKEN_MINUS,
            '*' => TOKEN_ASTERISK,
            '/' => TOKEN_SLASH,

            c if c.is_ascii_digit() => {
                self.consume_while(|c| c.is_ascii_digit());

                if self.consume_char('.') {
                    self.consume_while(|c| c.is_ascii_digit());
                    TOKEN_FLOAT
                } else {
                    TOKEN_INTEGER
                }
            },

            _ => TOKEN_ERROR,
        })
    }
}

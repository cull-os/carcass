use std::ptr;

use crate::SyntaxKind::{
    self,
    *,
};

#[derive(Debug, Clone)]
pub struct Token<'a>(pub SyntaxKind, pub &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerContext {
    StringishBody { delimiter: char },
    StringishEnd { delimiter: char },
    InterpolationStart,
    Interpolation { brackets: u32 },
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

fn is_valid_identifier_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-')
}

pub struct Tokenizer<'a> {
    context: Vec<TokenizerContext>,
    state: TokenizerState<'a>,
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let start_state = self.state.clone();

        self.next_kind()
            .map(|kind| Token(kind, self.consumed_since(start_state)))
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

    fn peek_character(&self) -> Option<char> {
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

    fn consume_character(&mut self, pattern: char) -> bool {
        let starts_with = Some(pattern) == self.peek_character();

        if starts_with {
            self.state.offset += pattern.len_utf8();
        }

        starts_with
    }

    fn consume_string(&mut self, pattern: &str) -> bool {
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

    fn next_character(&mut self) -> Option<char> {
        let next = self.peek_character()?;
        self.state.offset += next.len_utf8();
        Some(next)
    }

    fn next_string(&mut self, delimiter: char) -> Option<SyntaxKind> {
        loop {
            if self.peek_character() == Some(delimiter) {
                self.context_pop(TokenizerContext::StringishBody { delimiter });
                self.context_push(TokenizerContext::StringishEnd { delimiter });

                return Some(match delimiter {
                    '`' => TOKEN_IDENTIFIER_CONTENT,
                    '"' => TOKEN_STRING_CONTENT,
                    _ => unreachable!(),
                });
            }

            let start_state = self.state.clone();

            match self.next_character() {
                Some('\\') => {
                    match self.next_character() {
                        Some(_) => (),
                        None => {
                            self.context_pop(TokenizerContext::StringishBody { delimiter });
                            return Some(TOKEN_ERROR);
                        },
                    }
                },

                Some('$') if self.consume_character('{') => {
                    self.state = start_state;
                    self.context_push(TokenizerContext::InterpolationStart);

                    return Some(match delimiter {
                        '`' => TOKEN_IDENTIFIER_CONTENT,
                        '"' => TOKEN_STRING_CONTENT,
                        _ => unreachable!(),
                    });
                },

                Some(_) => (),
                None => {
                    self.context_pop(TokenizerContext::StringishBody { delimiter });
                    return Some(TOKEN_ERROR);
                },
            }
        }
    }

    fn next_kind(&mut self) -> Option<SyntaxKind> {
        let start_state = self.state.clone();

        match self.context.last() {
            Some(TokenizerContext::StringishBody { delimiter }) => {
                return self.next_string(*delimiter);
            },
            Some(TokenizerContext::StringishEnd { delimiter }) => {
                let delimiter = *delimiter;

                if !self.consume_character(delimiter) {
                    unreachable!()
                }

                self.context_pop(TokenizerContext::StringishEnd { delimiter });
                return Some(match delimiter {
                    '`' => TOKEN_IDENTIFIER_END,
                    '"' => TOKEN_STRING_END,
                    _ => unreachable!(),
                });
            },

            Some(TokenizerContext::InterpolationStart) => {
                if !self.consume_string("${") {
                    unreachable!()
                }

                self.context_pop(TokenizerContext::InterpolationStart);
                self.context_push(TokenizerContext::Interpolation { brackets: 0 });
                return Some(TOKEN_INTERPOLATION_START);
            },

            Some(TokenizerContext::Interpolation { .. }) | None => {},
        }

        Some(match self.next_character()? {
            c if c.is_whitespace() => {
                self.consume_while(char::is_whitespace);
                TOKEN_WHITESPACE
            },

            '#' => {
                // ### or more gets multiline
                if self.consume_while(|c| c == '#') >= 2 {
                    let end_delimiter = self.consumed_since(start_state);

                    let Some(end_after) = self.remaining().find(end_delimiter) else {
                        self.state.offset = self.state.input.len();
                        return Some(TOKEN_COMMENT);
                    };

                    self.state.offset += end_after + end_delimiter.len();
                } else {
                    self.consume_while(|c| c != '\r' && c != '\n');
                }

                TOKEN_COMMENT
            },

            '(' => TOKEN_LEFT_PARENTHESIS,
            ')' => TOKEN_RIGHT_PARENTHESIS,

            '+' if self.consume_character('+') => TOKEN_PLUS_PLUS,
            '[' => TOKEN_LEFT_BRACKET,
            ']' => TOKEN_RIGHT_BRACKET,

            '=' if self.consume_character('>') => TOKEN_EQUAL_MORE,
            '/' if self.consume_character('/') => TOKEN_SLASH_SLASH,
            '.' => TOKEN_PERIOD,
            '{' => {
                if let Some(TokenizerContext::Interpolation { brackets }) = self.context.last_mut()
                {
                    *brackets += 1
                }

                TOKEN_LEFT_CURLYBRACE
            },
            '}' => {
                if let Some(TokenizerContext::Interpolation { brackets }) = self.context.last_mut()
                {
                    match brackets.checked_sub(1) {
                        Some(new) => *brackets = new,
                        None => {
                            self.context_pop(TokenizerContext::Interpolation { brackets: 0 });
                            return Some(TOKEN_INTERPOLATION_END);
                        },
                    }
                }

                TOKEN_RIGHT_CURLYBRACE
            },
            '?' => TOKEN_QUESTIONMARK,
            ';' => TOKEN_SEMICOLON,

            '=' if self.consume_character('=') => TOKEN_EQUAL_EQUAL,
            '=' => TOKEN_EQUAL,
            '!' if self.consume_character('=') => TOKEN_EXCLAMATION_EQUAL,
            '<' if self.consume_character('=') => TOKEN_LESS_EQUAL,
            '<' => TOKEN_LESS,
            '>' if self.consume_character('=') => TOKEN_MORE_EQUAL,
            '>' => TOKEN_MORE,
            '-' if self.consume_character('>') => TOKEN_MINUS_GREATER,

            '@' => TOKEN_AT,
            ':' => TOKEN_COLON,

            '+' => TOKEN_PLUS,
            '-' => TOKEN_MINUS,
            '*' => TOKEN_ASTERISK,
            '/' => TOKEN_SLASH,

            c if c.is_ascii_digit() => {
                self.consume_while(|c| c.is_ascii_digit());

                if self.consume_character('.') {
                    self.consume_while(|c| c.is_ascii_digit());
                    TOKEN_FLOAT
                } else {
                    TOKEN_INTEGER
                }
            },

            c if is_valid_identifier_character(c) => {
                self.consume_while(is_valid_identifier_character);

                match self.consumed_since(start_state) {
                    "if" => TOKEN_LITERAL_IF,
                    "then" => TOKEN_LITERAL_THEN,
                    "else" => TOKEN_LITERAL_ELSE,
                    "and" => TOKEN_LITERAL_AND,
                    "or" => TOKEN_LITERAL_OR,
                    "not" => TOKEN_LITERAL_NOT,
                    _ => TOKEN_IDENTIFIER,
                }
            },

            delimiter @ ('`' | '"') => {
                self.context_push(TokenizerContext::StringishBody { delimiter });

                match delimiter {
                    '`' => TOKEN_IDENTIFIER_START,
                    '"' => TOKEN_STRING_START,
                    _ => unreachable!(),
                }
            },

            _ => TOKEN_ERROR,
        })
    }
}

use crate::{
    Kind,
    Kind::*,
};

fn is_valid_initial_identifier_character(c: char) -> bool {
    !c.is_ascii_digit() && c.is_alphanumeric() || matches!(c, '_' | '-')
}

fn is_valid_identifier_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-')
}

fn is_valid_path_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '/' | '_' | '-' | '\\' | '$')
}

/// Returns an iterator of tokens that reference the given string.
pub fn tokenize(input: &str) -> impl Iterator<Item = (Kind, &str)> {
    Tokenizer::new(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenizerContext<'a> {
    Path,
    Stringlike { end: &'a str },
    StringlikeEnd { end: &'a str },
    InterpolationStart,
    Interpolation { brackets: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tokenizer<'a> {
    input: &'a str,
    offset: usize,

    context: Vec<TokenizerContext<'a>>,
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = (Kind, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        let start_offset = self.offset;

        self.consume_kind().and_then(|kind| {
            match self.consumed_since(start_offset) {
                "" => self.next(),
                slice => Some((kind, slice)),
            }
        })
    }
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            offset: 0,
            context: Vec::new(),
        }
    }

    fn context_push(&mut self, context: TokenizerContext<'a>) {
        self.context.push(context)
    }

    fn context_pop(&mut self, context: TokenizerContext) {
        debug_assert_eq!(self.context.last(), Some(&context));
        self.context.pop();
    }

    fn remaining(&self) -> &str {
        &self.input[self.offset..]
    }

    fn peek_character(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn consume_while(&mut self, predicate: fn(char) -> bool) -> usize {
        let length: usize = self
            .remaining()
            .chars()
            .take_while(|c| predicate(*c))
            .map(char::len_utf8)
            .sum();

        self.offset += length;
        length
    }

    fn try_consume_character(&mut self, pattern: char) -> bool {
        let starts_with = self.peek_character() == Some(pattern);

        if starts_with {
            self.offset += pattern.len_utf8();
        }

        starts_with
    }

    fn try_consume_string(&mut self, pattern: &str) -> bool {
        let starts_with = self.remaining().starts_with(pattern);

        if starts_with {
            self.offset += pattern.len();
        }

        starts_with
    }

    fn consumed_since(&self, past_offset: usize) -> &'a str {
        &self.input[past_offset..self.offset]
    }

    fn consume_character(&mut self) -> Option<char> {
        let next = self.peek_character()?;
        self.offset += next.len_utf8();
        Some(next)
    }

    fn consume_stringlike(&mut self, end: &'a str) -> Option<Kind> {
        loop {
            if self.remaining().starts_with(end) {
                self.context_pop(TokenizerContext::Stringlike { end });
                self.context_push(TokenizerContext::StringlikeEnd { end });

                return Some(TOKEN_CONTENT);
            }

            let start_offset = self.offset;

            match self.consume_character() {
                Some('\\') => {
                    if self.consume_character().is_none() {
                        self.context_pop(TokenizerContext::Stringlike { end });
                        return Some(TOKEN_ERROR);
                    }
                },

                Some('$') if self.try_consume_character('{') => {
                    self.offset = start_offset;
                    self.context_push(TokenizerContext::InterpolationStart);

                    return Some(TOKEN_CONTENT);
                },

                Some(_) => {},
                None => {
                    self.context_pop(TokenizerContext::Stringlike { end });
                    return Some(TOKEN_ERROR);
                },
            }
        }
    }

    fn consume_path(&mut self) -> Option<Kind> {
        loop {
            if self
                .peek_character()
                .map_or(true, |c| !is_valid_path_character(c))
            {
                self.context_pop(TokenizerContext::Path);

                return Some(TOKEN_PATH);
            }

            let start_offset = self.offset;

            match self
                .consume_character()
                .expect("because end of file is handled by the if above")
            {
                '\\' => {
                    if self.consume_character().is_none() {
                        self.context_pop(TokenizerContext::Path);
                        return Some(TOKEN_ERROR);
                    }
                },

                '$' if self.try_consume_character('{') => {
                    self.offset = start_offset;
                    self.context_push(TokenizerContext::InterpolationStart);

                    return Some(TOKEN_PATH);
                },

                _ => {},
            }
        }
    }

    fn consume_kind(&mut self) -> Option<Kind> {
        let start_offset = self.offset;

        match self.context.last() {
            Some(TokenizerContext::Path) => {
                return self.consume_path();
            },

            Some(TokenizerContext::Stringlike { end }) => {
                return self.consume_stringlike(end);
            },
            Some(TokenizerContext::StringlikeEnd { end }) => {
                let end = *end;
                debug_assert!(self.try_consume_string(end));

                self.context_pop(TokenizerContext::StringlikeEnd { end });

                return Some(match end {
                    "`" => TOKEN_IDENTIFIER_END,
                    ">" => TOKEN_ISLAND_END,
                    _ => TOKEN_STRING_END,
                });
            },

            Some(TokenizerContext::InterpolationStart) => {
                debug_assert!(self.try_consume_string("${"));

                self.context_pop(TokenizerContext::InterpolationStart);
                self.context_push(TokenizerContext::Interpolation { brackets: 0 });
                return Some(TOKEN_INTERPOLATION_START);
            },

            Some(TokenizerContext::Interpolation { .. }) | None => {},
        }

        Some(match self.consume_character()? {
            c if c.is_whitespace() => {
                self.consume_while(char::is_whitespace);
                TOKEN_WHITESPACE
            },

            '#' => {
                // ### or more gets multiline
                if self.consume_while(|c| c == '#') >= 2 {
                    let end = self.consumed_since(start_offset);

                    let Some(end_after) = self.remaining().find(end) else {
                        // Don't have to close it, it just eats the whole file up.
                        self.offset = self.input.len();
                        return Some(TOKEN_COMMENT);
                    };

                    self.offset += end_after + end.len();
                } else {
                    self.consume_while(|c| !matches!(c, '\r' | '\n'));
                }

                TOKEN_COMMENT
            },

            '$' => TOKEN_DOLLAR,
            '|' if self.try_consume_character('>') => TOKEN_PIPE_MORE,

            '(' => TOKEN_LEFT_PARENTHESIS,
            ')' => TOKEN_RIGHT_PARENTHESIS,

            '+' if self.try_consume_character('+') => TOKEN_PLUS_PLUS,
            '[' => TOKEN_LEFT_BRACKET,
            ']' => TOKEN_RIGHT_BRACKET,

            '=' if self.try_consume_string("=>") => TOKEN_EQUAL_EQUAL_MORE,
            '<' if self.try_consume_string("==") => TOKEN_LESS_EQUAL_EQUAL,
            '/' if self.try_consume_character('/') => TOKEN_SLASH_SLASH,
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

            '=' if self.try_consume_character('=') => TOKEN_EQUAL_EQUAL,
            '=' => TOKEN_EQUAL,
            '!' if self.try_consume_character('=') => TOKEN_EXCLAMATION_EQUAL,
            '>' if self.try_consume_character('=') => TOKEN_MORE_EQUAL,
            '>' => TOKEN_MORE,
            '-' if self.try_consume_character('>') => TOKEN_MINUS_MORE,

            '@' => TOKEN_AT,
            ',' => TOKEN_COMMA,
            ':' => TOKEN_COLON,

            '+' => TOKEN_PLUS,
            '-' => TOKEN_MINUS,
            '*' if self.try_consume_character('*') => TOKEN_ASTERISK_ASTERISK,
            '*' => TOKEN_ASTERISK,

            '0' if matches!(self.peek_character(), Some('b' | 'o' | 'x')) => {
                let is_valid_digit = match self.consume_character() {
                    Some('b') => |c: char| matches!(c, '0' | '1'),
                    Some('o') => |c: char| matches!(c, '0'..='7'),
                    Some('x') => |c: char| c.is_ascii_hexdigit(),
                    _ => unreachable!(),
                };

                let start = self.offset;
                self.consume_while(is_valid_digit);

                if self.offset - start == 0 {
                    TOKEN_ERROR
                } else {
                    TOKEN_INTEGER
                }
            },

            initial_digit if initial_digit.is_ascii_digit() => {
                self.consume_while(|c| c.is_ascii_digit());

                if self.try_consume_character('.') {
                    let start = self.offset;
                    self.consume_while(|c| c.is_ascii_digit());

                    if self.offset - start == 0 {
                        TOKEN_ERROR
                    } else {
                        TOKEN_FLOAT
                    }
                } else {
                    TOKEN_INTEGER
                }
            },

            '.' if self.peek_character().map_or(false, |c| c.is_ascii_digit()) => {
                self.consume_while(|c| c.is_ascii_digit());

                TOKEN_FLOAT
            },

            initial_letter if is_valid_initial_identifier_character(initial_letter) => {
                self.consume_while(is_valid_identifier_character);

                match self.consumed_since(start_offset) {
                    "if" => TOKEN_LITERAL_IF,
                    "then" => TOKEN_LITERAL_THEN,
                    "else" => TOKEN_LITERAL_ELSE,

                    "and" => TOKEN_LITERAL_AND,
                    "or" => TOKEN_LITERAL_OR,
                    "not" => TOKEN_LITERAL_NOT,

                    _ => TOKEN_IDENTIFIER,
                }
            },

            start @ ('`' | '"') => {
                self.context_push(TokenizerContext::Stringlike {
                    end: match start {
                        '`' => "`",
                        _ => "\"",
                    },
                });

                match start {
                    '`' => TOKEN_IDENTIFIER_START,
                    _ => TOKEN_STRING_START,
                }
            },

            '\'' => {
                self.consume_while(|c| c == '\'');

                self.context_push(TokenizerContext::Stringlike {
                    end: self.consumed_since(start_offset),
                });

                TOKEN_STRING_START
            },

            '.' if self
                .peek_character()
                .map_or(false, |c| matches!(c, '.' | '/')) =>
            {
                self.offset -= 1;
                self.context_push(TokenizerContext::Path);

                return self.consume_kind();
            },
            '/' if self.peek_character().map_or(false, is_valid_path_character) => {
                self.offset -= 1;
                self.context_push(TokenizerContext::Path);

                return self.consume_kind();
            },

            '.' => TOKEN_PERIOD,
            '/' => TOKEN_SLASH,

            '<' if self.peek_character().map_or(false, |c| {
                c == '$' || is_valid_initial_identifier_character(c)
            }) =>
            {
                self.context_push(TokenizerContext::Stringlike { end: ">" });

                TOKEN_ISLAND_START
            },

            '<' if self.try_consume_character('=') => TOKEN_LESS_EQUAL,
            '<' => TOKEN_LESS,

            _ => TOKEN_ERROR,
        })
    }
}

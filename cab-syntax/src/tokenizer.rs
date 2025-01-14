use crate::Kind::{
    self,
    *,
};

/// Returns an iterator of tokens that reference the given string.
pub fn tokenize(input: &str) -> impl Iterator<Item = (Kind, &str)> {
    Tokenizer::new(input)
}

fn is_valid_initial_identifier_character(c: char) -> bool {
    !c.is_ascii_digit() && is_valid_identifier_character(c)
}

fn is_valid_identifier_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-')
}

fn is_valid_path_character(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '/' | '_' | '-' | '\\')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerContext<'a> {
    Path,
    Stringlike { end: &'a str },
    StringlikeEnd { end: &'a str },
    InterpolationStart,
    Interpolation { parentheses: u32 },
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
        assert_eq!(self.context.last(), Some(&context));
        self.context.pop();
    }

    fn remaining(&self) -> &str {
        &self.input[self.offset..]
    }

    fn peek_character_nth(&self, n: usize) -> Option<char> {
        self.remaining().chars().nth(n)
    }

    fn peek_character(&self) -> Option<char> {
        self.peek_character_nth(0)
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

    fn consume_scientific(&mut self) -> Kind {
        if self.try_consume_character('e') || self.try_consume_character('E') {
            let _ = self.try_consume_character('+') || self.try_consume_character('-');

            if self.consume_while(|c| c.is_ascii_digit() || c == '_') == 0 {
                TOKEN_ERROR
            } else {
                TOKEN_FLOAT
            }
        } else {
            TOKEN_FLOAT
        }
    }

    fn consume_stringlike(&mut self, end: &'a str) -> Option<Kind> {
        loop {
            if self.remaining().starts_with(end) {
                self.context_pop(TokenizerContext::Stringlike { end });
                self.context_push(TokenizerContext::StringlikeEnd { end });

                return Some(TOKEN_CONTENT);
            }

            match self.peek_character() {
                Some('\\') if self.peek_character_nth(1) == Some('(') => {
                    self.context_push(TokenizerContext::InterpolationStart);

                    return Some(TOKEN_CONTENT);
                },

                Some(_) => {
                    if let Some('\\') = self.consume_character() {
                        self.consume_character();
                    };
                },

                None => {
                    self.context_pop(TokenizerContext::Stringlike { end });
                    return Some(TOKEN_CONTENT);
                },
            }
        }
    }

    fn consume_path(&mut self) -> Option<Kind> {
        loop {
            if self
                .peek_character()
                .is_none_or(|c| !is_valid_path_character(c))
            {
                self.context_pop(TokenizerContext::Path);

                return Some(TOKEN_PATH);
            }

            match self.peek_character().unwrap() {
                '\\' if self.peek_character_nth(1) == Some('(') => {
                    self.context_push(TokenizerContext::InterpolationStart);

                    return Some(TOKEN_PATH);
                },

                _ => {
                    if let Some('\\') = self.consume_character() {
                        self.consume_character();
                    };
                },
            }
        }
    }

    fn consume_kind(&mut self) -> Option<Kind> {
        let start_offset = self.offset;

        match self.context.last().copied() {
            Some(TokenizerContext::Path) => {
                return self.consume_path();
            },

            Some(TokenizerContext::Stringlike { end }) => {
                return self.consume_stringlike(end);
            },
            Some(TokenizerContext::StringlikeEnd { end }) => {
                assert!(self.try_consume_string(end));

                self.context_pop(TokenizerContext::StringlikeEnd { end });

                return Some(match end {
                    "`" => TOKEN_IDENTIFIER_END,
                    ">" => TOKEN_ISLAND_END,
                    _ => TOKEN_STRING_END,
                });
            },

            Some(TokenizerContext::InterpolationStart) => {
                assert!(self.try_consume_string(r"\("));

                self.context_pop(TokenizerContext::InterpolationStart);
                self.context_push(TokenizerContext::Interpolation { parentheses: 0 });
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
                // ### or more is multiline.
                if self.consume_while(|c| c == '#') < 2 {
                    self.consume_while(|c| !matches!(c, '\r' | '\n'));
                } else {
                    let end = self.consumed_since(start_offset);

                    let Some(end_after) = self.remaining().find(end) else {
                        // Don't have to close it, it just eats the whole file up.
                        self.offset = self.input.len();
                        return Some(TOKEN_COMMENT);
                    };

                    self.offset += end_after + end.len();
                }

                TOKEN_COMMENT
            },

            ';' => TOKEN_SEMICOLON,

            '<' if self.try_consume_character('|') => TOKEN_LESS_PIPE,
            '|' if self.try_consume_character('>') => TOKEN_PIPE_MORE,

            '(' => {
                if let Some(TokenizerContext::Interpolation { parentheses }) =
                    self.context.last_mut()
                {
                    *parentheses += 1;
                }

                TOKEN_LEFT_PARENTHESIS
            },
            ')' => {
                if let Some(TokenizerContext::Interpolation { parentheses }) =
                    self.context.last_mut()
                {
                    match parentheses.checked_sub(1) {
                        Some(new) => *parentheses = new,
                        None => {
                            self.context_pop(TokenizerContext::Interpolation { parentheses: 0 });
                            return Some(TOKEN_INTERPOLATION_END);
                        },
                    }
                }

                TOKEN_RIGHT_PARENTHESIS
            },

            '+' if self.try_consume_character('+') => TOKEN_PLUS_PLUS,
            '[' => TOKEN_LEFT_BRACKET,
            ']' => TOKEN_RIGHT_BRACKET,

            '/' if self.try_consume_character('/') => TOKEN_SLASH_SLASH,
            '{' => TOKEN_LEFT_CURLYBRACE,
            '}' => TOKEN_RIGHT_CURLYBRACE,
            '?' => TOKEN_QUESTIONMARK,

            ':' if self.try_consume_character('=') => TOKEN_COLON_EQUAL,
            '=' if self.try_consume_character('>') => TOKEN_EQUAL_GREATER,
            ',' => TOKEN_COMMA,

            '!' if self.try_consume_character('=') => TOKEN_EXCLAMATION_EQUAL,
            '=' if self.try_consume_character('=') => TOKEN_EQUAL_EQUAL,
            '>' if self.try_consume_character('=') => TOKEN_MORE_EQUAL,
            '>' => TOKEN_MORE,

            '&' if self.try_consume_character('&') => TOKEN_AMPERSAND_AMPERSAND,
            '|' if self.try_consume_character('|') => TOKEN_PIPE_PIPE,
            '!' => TOKEN_EXCLAMATIONMARK,
            '-' if self.try_consume_character('>') => TOKEN_MINUS_MORE,

            '+' => TOKEN_PLUS,
            '-' => TOKEN_MINUS,
            '*' => TOKEN_ASTERISK,
            '^' => TOKEN_CARET,

            '0' if matches!(self.peek_character(), Some('b' | 'o' | 'x')) => {
                let is_valid_digit = match self.consume_character() {
                    Some('b') => |c: char| matches!(c, '0' | '1' | '_'),
                    Some('o') => |c: char| matches!(c, '0'..='7' | '_'),
                    Some('x') => |c: char| c.is_ascii_hexdigit() || c == '_',
                    _ => unreachable!(),
                };

                if self.consume_while(is_valid_digit) == 0 {
                    return Some(TOKEN_ERROR);
                }

                if self.peek_character() == Some('.')
                    && self.peek_character_nth(1).is_some_and(is_valid_digit)
                {
                    self.consume_character();
                    self.consume_while(is_valid_digit);
                    self.consume_scientific()
                } else {
                    TOKEN_INTEGER
                }
            },

            initial_digit if initial_digit.is_ascii_digit() => {
                let is_valid_digit = |c: char| c.is_ascii_digit() || c == '_';

                self.consume_while(is_valid_digit);

                if self.peek_character() == Some('.')
                    && self.peek_character_nth(1).is_some_and(is_valid_digit)
                {
                    self.consume_character();
                    self.consume_while(is_valid_digit);
                    self.consume_scientific()
                } else {
                    TOKEN_INTEGER
                }
            },

            initial_letter if is_valid_initial_identifier_character(initial_letter) => {
                self.consume_while(is_valid_identifier_character);

                const KEYWORDS: phf::Map<&'static str, Kind> = phf::phf_map! {
                    "if" => TOKEN_LITERAL_IF,
                    "is" => TOKEN_LITERAL_IS,
                    "then" => TOKEN_LITERAL_THEN,
                    "else" => TOKEN_LITERAL_ELSE,
                };

                KEYWORDS
                    .get(self.consumed_since(start_offset))
                    .copied()
                    .unwrap_or(TOKEN_IDENTIFIER)
            },

            '.' if matches!(self.peek_character(), Some('.' | '/')) => {
                self.offset -= 1;
                self.context_push(TokenizerContext::Path);

                return self.consume_kind();
            },
            '/' if self.peek_character().is_some_and(is_valid_path_character) => {
                self.offset -= 1;
                self.context_push(TokenizerContext::Path);

                return self.consume_kind();
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

            '.' => TOKEN_PERIOD,
            '/' => TOKEN_SLASH,

            '<' if self
                .peek_character()
                .is_some_and(|c| is_valid_initial_identifier_character(c) || c == '\\') =>
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

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_token_matches {
        ($string:literal, $($pattern:pat),* $(,)?) => {{
            let mut tokens = tokenize($string);

            $(assert!(matches!(tokens.next(), Some($pattern)));)*

            assert_eq!(tokens.next(), None);
        }};
    }

    #[test]
    fn no_empty_tokens() {
        assert_token_matches!(
            r"'foo \(bar)'",
            (TOKEN_STRING_START, "'"),
            (TOKEN_CONTENT, "foo "),
            (TOKEN_INTERPOLATION_START, r"\("),
            (TOKEN_IDENTIFIER, "bar"),
            (TOKEN_INTERPOLATION_END, ")"),
            (TOKEN_STRING_END, "'"),
        );
    }

    #[test]
    fn path() {
        assert_token_matches!(
            r"../foo\(𓃰)///baz",
            (TOKEN_PATH, "../foo"),
            (TOKEN_INTERPOLATION_START, r"\("),
            (TOKEN_IDENTIFIER, "𓃰"),
            (TOKEN_INTERPOLATION_END, ")"),
            (TOKEN_PATH, "///baz"),
        );
    }

    #[test]
    fn errors_are_individual() {
        assert_token_matches!(
            "~~~",
            (TOKEN_ERROR, "~"),
            (TOKEN_ERROR, "~"),
            (TOKEN_ERROR, "~")
        );
    }
}

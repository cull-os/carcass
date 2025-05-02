use std::assert_matches::assert_matches;

use smallvec::SmallVec;

use crate::Kind::{
   self,
   *,
};

/// Returns an iterator of tokens that reference the given string.
pub fn tokenize(source: &str) -> impl Iterator<Item = (Kind, &str)> {
   Tokenizer::new(source)
}

fn is_valid_initial_identifier_character(c: char) -> bool {
   let invalid = c.is_ascii_digit() || c == '-' || c == '\'';

   !invalid && is_valid_identifier_character(c)
}

fn is_valid_identifier_character(c: char) -> bool {
   c.is_alphanumeric() || matches!(c, '_' | '-' | '\'')
}

fn is_valid_path_character(c: char) -> bool {
   c.is_alphanumeric() || matches!(c, '.' | '/' | '_' | '-' | '\\')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Context<'a> {
   Path,
   PathEnd,
   PathRootType,
   PathRootTypeEnd,

   Delimited {
      before: Option<&'a str>,
      end:    char,
   },
   DelimitedEnd {
      before: Option<&'a str>,
      end:    char,
   },

   InterpolationStart,
   Interpolation {
      parentheses: usize,
   },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Tokenizer<'a> {
   source: &'a str,
   offset: usize,

   context: SmallVec<Context<'a>, 4>,
}

impl<'a> Iterator for Tokenizer<'a> {
   type Item = (Kind, &'a str);

   fn next(&mut self) -> Option<Self::Item> {
      let start = self.offset;

      self
         .consume_kind()
         .map(|kind| (kind, self.consumed_since(start)))
   }
}

impl<'a> Tokenizer<'a> {
   fn new(source: &'a str) -> Self {
      Self {
         source,
         offset: 0,
         context: SmallVec::new(),
      }
   }

   fn context_push(&mut self, context: Context<'a>) {
      self.context.push(context);
   }

   fn context_pop(&mut self, context: Context) {
      assert_eq!(self.context.last(), Some(&context));
      self.context.pop();
   }

   fn remaining(&self) -> &str {
      &self.source[self.offset..]
   }

   #[expect(clippy::min_ident_chars)]
   fn peek_character_nth(&self, n: usize) -> Option<char> {
      self.remaining().chars().nth(n)
   }

   fn peek_character(&self) -> Option<char> {
      self.peek_character_nth(0)
   }

   fn consume_while(&mut self, predicate: fn(char) -> bool) -> usize {
      let len: usize = self
         .remaining()
         .chars()
         .take_while(|&c| predicate(c))
         .map(char::len_utf8)
         .sum();

      self.offset += len;
      len
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
      &self.source[past_offset..self.offset]
   }

   fn consume_character(&mut self) -> Option<char> {
      let next = self.peek_character()?;
      self.offset += next.len_utf8();
      Some(next)
   }

   fn consume_delimited_part(&mut self) -> Option<Kind> {
      match self.peek_character().unwrap() {
         '\\' if self.peek_character_nth(1) == Some('(') => {
            self.context_push(Context::InterpolationStart);

            Some(TOKEN_CONTENT)
         },

         '\\' => {
            self.consume_character();
            self.consume_character();

            None
         },

         _ => {
            self.consume_character();

            None
         },
      }
   }

   fn consume_delimited(&mut self, before: Option<&'a str>, end: char) -> Kind {
      loop {
         let remaining = self.remaining();

         if before.is_none_or(|before| remaining.starts_with(before))
            && remaining
               .get(before.map_or(0, str::len)..)
               .is_some_and(|remaining| remaining.starts_with(end))
         {
            self.context_pop(Context::Delimited { before, end });
            self.context_push(Context::DelimitedEnd { before, end });

            return TOKEN_CONTENT;
         }

         if self.peek_character().is_none() {
            self.context_pop(Context::Delimited { before, end });

            return TOKEN_CONTENT;
         }

         if let Some(kind) = self.consume_delimited_part() {
            return kind;
         }
      }
   }

   fn consume_path_root_type(&mut self) -> Kind {
      loop {
         if let Some('>' | ':') = self.peek_character() {
            self.context_pop(Context::PathRootType);
            self.context_push(Context::PathRootTypeEnd);

            return TOKEN_CONTENT;
         }

         if self.peek_character().is_none() {
            self.context_pop(Context::PathRootType);

            return TOKEN_CONTENT;
         }

         if let Some(kind) = self.consume_delimited_part() {
            return kind;
         }
      }
   }

   fn consume_path(&mut self) -> Kind {
      loop {
         if self
            .peek_character()
            .is_none_or(|c| !is_valid_path_character(c))
         {
            self.context_pop(Context::Path);
            self.context_push(Context::PathEnd);

            return TOKEN_CONTENT;
         }

         if let Some(kind) = self.consume_delimited_part() {
            return kind;
         }
      }
   }

   fn consume_scientific(&mut self) -> Kind {
      if self.try_consume_character('e') || self.try_consume_character('E') {
         let _ = self.try_consume_character('+') || self.try_consume_character('-');

         let exponent_len = self.consume_while(|c| c.is_ascii_digit() || c == '_');
         let exponent = self.consumed_since(self.offset - exponent_len);
         if exponent.is_empty() || exponent.bytes().all(|c| c == b'_') {
            TOKEN_ERROR_FLOAT_NO_EXPONENT
         } else {
            TOKEN_FLOAT
         }
      } else {
         TOKEN_FLOAT
      }
   }

   fn consume_kind(&mut self) -> Option<Kind> {
      let start = self.offset;

      match self.context.last().copied() {
         Some(Context::PathRootType) => {
            return Some(self.consume_path_root_type());
         },
         Some(Context::PathRootTypeEnd) => {
            assert_matches!(self.consume_character(), Some('>' | ':'));
            self.context_pop(Context::PathRootTypeEnd);

            return Some(TOKEN_PATH_ROOT_TYPE_END);
         },

         Some(Context::Path) => {
            return Some(self.consume_path());
         },
         Some(Context::PathEnd) => {
            self.context_pop(Context::PathEnd);

            return Some(TOKEN_PATH_END);
         },

         Some(Context::Delimited { before, end }) => {
            return Some(self.consume_delimited(before, end));
         },
         Some(Context::DelimitedEnd { before, end }) => {
            if let Some(before) = before {
               assert!(self.try_consume_string(before));
            }
            assert_eq!(self.consume_character(), Some(end));

            self.context_pop(Context::DelimitedEnd { before, end });

            return Some(match end {
               '`' => TOKEN_QUOTED_IDENTIFIER_END,
               '"' => TOKEN_STRING_END,
               '\'' => TOKEN_RUNE_END,
               _ => unreachable!(),
            });
         },

         Some(Context::InterpolationStart) => {
            assert!(self.try_consume_string(r"\("));

            self.context_pop(Context::InterpolationStart);
            self.context_push(Context::Interpolation { parentheses: 0 });
            return Some(TOKEN_INTERPOLATION_START);
         },
         Some(Context::Interpolation { .. }) => {},

         None => {},
      }

      Some(match self.consume_character()? {
         c if c.is_whitespace() => {
            self.consume_while(char::is_whitespace);

            TOKEN_WHITESPACE
         },

         '#' if self.peek_character() == Some('=') => {
            let equals_len = self.consume_while(|c| c == '=');
            let equals = self.consumed_since(self.offset - equals_len);

            loop {
               match self.peek_character() {
                  Some('=')
                     if let remaining = self.remaining()
                        && remaining.starts_with(equals)
                        && remaining.as_bytes().get(equals_len).copied() == Some(b'#') =>
                  {
                     // Hard code a 1 here because that comparison up top is a byte.
                     self.offset += equals_len + 1;

                     break TOKEN_COMMENT;
                  },

                  // #= ==# is not a closed comment.
                  Some('=') => {
                     self.consume_while(|c| c == '=');
                  },

                  Some('#') if self.peek_character_nth(1) == Some('=') => {
                     self.consume_kind();
                  },

                  Some(_) => {
                     self.consume_character();
                  },

                  None => {
                     break TOKEN_COMMENT;
                  },
               }
            }
         },

         '#' => {
            self.consume_while(|c| !matches!(c, '\n'));

            TOKEN_COMMENT
         },

         ',' => TOKEN_COMMA,
         ';' => TOKEN_SEMICOLON,

         '<' if self.try_consume_character('|') => TOKEN_LESS_PIPE,
         '|' if self.try_consume_character('>') => TOKEN_PIPE_MORE,

         '(' if let Some(&mut Context::Interpolation {
            ref mut parentheses,
         }) = self.context.last_mut() =>
         {
            *parentheses += 1;
            TOKEN_PARENTHESIS_LEFT
         },
         ')' if let Some(&mut Context::Interpolation {
            ref mut parentheses,
         }) = self.context.last_mut() =>
         {
            match parentheses.checked_sub(1) {
               Some(new) => {
                  *parentheses = new;
                  TOKEN_PARENTHESIS_RIGHT
               },

               None => {
                  self.context_pop(Context::Interpolation { parentheses: 0 });
                  TOKEN_INTERPOLATION_END
               },
            }
         },

         '(' => TOKEN_PARENTHESIS_LEFT,
         ')' => TOKEN_PARENTHESIS_RIGHT,

         '=' if self.try_consume_character('>') => TOKEN_EQUAL_MORE,

         ':' => TOKEN_COLON,
         '+' if self.try_consume_character('+') => TOKEN_PLUS_PLUS,
         '[' => TOKEN_BRACKET_LEFT,
         ']' => TOKEN_BRACKET_RIGHT,

         '/' if self.try_consume_character('/') => TOKEN_SLASH_SLASH,
         '{' => TOKEN_CURLYBRACE_LEFT,
         '}' => TOKEN_CURLYBRACE_RIGHT,

         '>' if self.try_consume_character('=') => TOKEN_MORE_EQUAL,
         '>' => TOKEN_MORE,

         '!' if self.try_consume_character('=') => TOKEN_EXCLAMATION_EQUAL,
         '=' => TOKEN_EQUAL,

         '&' if self.try_consume_character('&') => TOKEN_AMPERSAND_AMPERSAND,
         '|' if self.try_consume_character('|') => TOKEN_PIPE_PIPE,
         '!' => TOKEN_EXCLAMATION,
         '-' if self.try_consume_character('>') => TOKEN_MINUS_MORE,

         '&' => TOKEN_AMPERSAND,
         '|' => TOKEN_PIPE,

         '+' => TOKEN_PLUS,
         '-' => TOKEN_MINUS,
         '*' => TOKEN_ASTERISK,
         '^' => TOKEN_CARET,

         '0' if let Some('b' | 'B' | 'o' | 'O' | 'x' | 'X') = self.peek_character() => {
            let is_valid_digit = match self.consume_character() {
               Some('b' | 'B') => |c: char| matches!(c, '0' | '1' | '_'),
               Some('o' | 'O') => |c: char| matches!(c, '0'..='7' | '_'),
               Some('x' | 'X') => |c: char| c.is_ascii_hexdigit() || c == '_',
               _ => unreachable!(),
            };

            let digits_len = self.consume_while(is_valid_digit);
            let digits = self.consumed_since(self.offset - digits_len);
            let error_token = (digits.is_empty() || digits.bytes().all(|c| c == b'_'))
               .then_some(TOKEN_ERROR_NUMBER_NO_DIGIT);

            if self.peek_character() == Some('.')
               && self.peek_character_nth(1).is_some_and(is_valid_digit)
            {
               self.consume_character();
               self.consume_while(is_valid_digit);
               error_token.unwrap_or(self.consume_scientific())
            } else {
               error_token.unwrap_or(TOKEN_INTEGER)
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
            const KEYWORDS: phf::Map<&'static str, Kind> = phf::phf_map! {
                "if" => TOKEN_KEYWORD_IF,
                "then" => TOKEN_KEYWORD_THEN,
                "else" => TOKEN_KEYWORD_ELSE,
            };

            self.consume_while(is_valid_identifier_character);

            KEYWORDS
               .get(self.consumed_since(start))
               .copied()
               .unwrap_or(TOKEN_IDENTIFIER)
         },

         // \(foo)/bar/baz.txt
         start @ '\\' => {
            self.offset -= start.len_utf8();
            self.context_push(Context::Path);

            TOKEN_PATH_CONTENT_START
         },
         // ./bar/baz.txt
         start @ '.' if let Some('.' | '/') = self.peek_character() => {
            self.offset -= start.len_utf8();
            self.context_push(Context::Path);

            TOKEN_PATH_CONTENT_START
         },
         // /bar/baz.txt
         start @ '/' if self.peek_character().is_some_and(is_valid_path_character) => {
            self.offset -= start.len_utf8();
            self.context_push(Context::Path);

            TOKEN_PATH_CONTENT_START
         },
         // <self>/bar/baz.txt
         '<' if self
            .peek_character()
            .is_some_and(|c| is_valid_initial_identifier_character(c) || c == '\\') =>
         {
            self.context_push(Context::PathRootType);

            TOKEN_PATH_ROOT_TYPE_START
         },

         '@' => TOKEN_AT,

         start @ ('`' | '"' | '\'') => {
            let equals_len = self.consume_while(|c| c == '=');
            let equals = self.consumed_since(self.offset - equals_len);

            self.context_push(Context::Delimited {
               before: Some(equals),
               end:    start,
            });

            match start {
               '`' => TOKEN_QUOTED_IDENTIFIER_START,
               '\"' => TOKEN_STRING_START,
               '\'' => TOKEN_RUNE_START,
               _ => unreachable!(),
            }
         },

         // TODO: Reorder `Syntax` and parse these in the same order.
         '.' => TOKEN_PERIOD,
         '/' => TOKEN_SLASH,

         '<' if self.try_consume_character('=') => TOKEN_LESS_EQUAL,
         '<' => TOKEN_LESS,

         _ => TOKEN_ERROR_UNKNOWN,
      })
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   macro_rules! assert_token_matches {
      ($string:literal, $($pattern:pat),* $(,)?) => {{
         let mut tokens = tokenize($string);

         $(assert_matches!(tokens.next(), Some($pattern));)*

         assert_matches!(tokens.next(), None);
      }};
   }

   #[test]
   fn empty_tokens() {
      assert_token_matches!(
         r#""foo \(bar)""#,
         (TOKEN_STRING_START, r#"""#),
         (TOKEN_CONTENT, "foo "),
         (TOKEN_INTERPOLATION_START, r"\("),
         (TOKEN_IDENTIFIER, "bar"),
         (TOKEN_INTERPOLATION_END, ")"),
         (TOKEN_CONTENT, ""),
         (TOKEN_STRING_END, r#"""#),
      );
   }

   #[test]
   fn number_errors() {
      assert_token_matches!(
         "0b__e 0x0 0x123.0e 0o777.0e",
         (TOKEN_ERROR_NUMBER_NO_DIGIT, "0b__"),
         (TOKEN_IDENTIFIER, "e"),
         (TOKEN_WHITESPACE, " "),
         (TOKEN_INTEGER, "0x0"),
         (TOKEN_WHITESPACE, " "),
         (TOKEN_FLOAT, "0x123.0e"), // e is a valid hexadecimal digit.
         (TOKEN_WHITESPACE, " "),
         (TOKEN_ERROR_FLOAT_NO_EXPONENT, "0o777.0e")
      );
   }

   #[test]
   fn path() {
      assert_token_matches!(
         r"../foo\(𓃰)///baz",
         (TOKEN_PATH_CONTENT_START, ""),
         (TOKEN_CONTENT, "../foo"),
         (TOKEN_INTERPOLATION_START, r"\("),
         (TOKEN_IDENTIFIER, "𓃰"),
         (TOKEN_INTERPOLATION_END, ")"),
         (TOKEN_CONTENT, "///baz"),
         (TOKEN_PATH_END, ""),
      );
   }

   #[test]
   fn errors_are_individual() {
      assert_token_matches!(
         "~~~",
         (TOKEN_ERROR_UNKNOWN, "~"),
         (TOKEN_ERROR_UNKNOWN, "~"),
         (TOKEN_ERROR_UNKNOWN, "~")
      );
   }
}

//! Typed [`Token`] definitions.
//!
//! [`Token`]: crate::Token
use std::{
   fmt,
   ptr,
};

use derive_more::Deref;
use num::{
   Num as _,
   bigint as num_bigint,
   traits as num_traits,
};
use ranged::{
   IntoSize as _,
   Span,
};
use smallvec::SmallVec;
use ust::style::{
   self,
   StyledExt as _,
};

use crate::{
   Kind::*,
   red,
};

/// Returns whether this identifier can be represented without quotes.
#[must_use]
pub fn is_valid_plain_identifier(s: &str) -> bool {
   let mut chars = s.chars();

   chars
      .by_ref()
      .next()
      .is_some_and(is_valid_initial_plain_identifier_character)
      && chars.all(is_valid_plain_identifier_character)
}

#[must_use]
pub fn is_valid_initial_plain_identifier_character(c: char) -> bool {
   let invalid = c.is_ascii_digit() || c == '-' || c == '\'';

   !invalid && is_valid_plain_identifier_character(c)
}

#[must_use]
pub fn is_valid_plain_identifier_character(c: char) -> bool {
   c.is_alphanumeric() || matches!(c, '_' | '-' | '\'')
}

#[must_use]
pub fn is_valid_path_character(c: char) -> bool {
   c.is_alphanumeric() || matches!(c, '.' | '/' | '_' | '-' | '\\' | '(' | ')')
}

#[must_use]
pub fn unescape(c: char) -> Option<char> {
   Some(match c {
      ' ' => ' ',
      '0' => '\x00', // Null.
      'a' => '\x07', // Bell.
      'b' => '\x08', // Backspace.
      't' => '\x09', // Horizontal tab.
      'n' => '\x0A', // New line.
      'v' => '\x0B', // Vertical tab.
      'f' => '\x0C', // Form feed.
      'r' => '\x0D', // Carriage return.
      '=' => '=',
      '`' => '`',
      '"' => '\"',
      '\'' => '\'',
      '\\' => '\\',

      _ => return None,
   })
}

pub fn unescape_string(s: &str) -> Result<(String, bool), SmallVec<Span, 4>> {
   let mut string = String::with_capacity(s.len());
   let mut escaped_newline = false;
   let mut invalids = SmallVec::<Span, 4>::new();

   let mut chars = s.char_indices().peekable();
   while let Some((index, c)) = chars.next() {
      if c != '\\' {
         string.push(c);
         continue;
      }

      let Some((_, next)) = chars.next() else {
         // When a string ends with '\', it has to be followed by a newline.
         // And that escapes the newline.
         escaped_newline = true;
         continue;
      };

      let Some(unescaped) = unescape(next) else {
         invalids.push(Span::at(index, '\\'.size() + next.size()));
         continue;
      };

      string.push(unescaped);
   }

   if invalids.is_empty() {
      Ok((string, escaped_newline))
   } else {
      Err(invalids)
   }
}

#[bon::builder]
pub fn escape(
   #[builder(start_fn)] c: char,
   delimiter: Option<(char, &'static str)>,
   is_first: bool,
) -> Option<&'static str> {
   Some(match c {
      // Turn one line of the `unescape` match to an `escape` match in Helix.
      // Copy this to your @ register using "@y. Execute using Q.
      // gst,<S-S><space>=<gt><space><ret><A-)>,t,<right><left><left>mr'"i\\<esc>gs
      '\x00' => "\\0", // Null.
      '\x07' => "\\a", // Bell.
      '\x08' => "\\b", // Backspace.
      '\x09' => "\\t", // Horizontal tab.
      '\x0A' => "\\n", // New line.
      '\x0B' => "\\v", // Vertical tab.
      '\x0C' => "\\f", // Form feed.
      '\x0D' => "\\r", // Carriage return.

      c if let Some((delimiter, delimiter_escaped)) = delimiter
         && c == delimiter =>
      {
         delimiter_escaped
      },

      // "=" is not a valid string, but "\=" is.
      // However, "\==" is also valid and we don't want to over-escape.
      '=' if is_first => "\\=",

      _ => return None,
   })
}

#[bon::builder]
pub fn escape_string<'a>(
   #[builder(start_fn)] s: &'a str,
   #[builder(default)] normal_style: style::Style,
   #[builder(default)] escaped_style: style::Style,
   delimiter: Option<(char, &'static str)>,
) -> impl Iterator<Item = style::Styled<&'a str>> {
   // Bon doesn't like generator syntax.
   escape_string_impl(s, normal_style, escaped_style, delimiter)
}

fn escape_string_impl<'a>(
   s: &'a str,
   normal: style::Style,
   escaped: style::Style,
   delimiter: Option<(char, &'static str)>,
) -> impl Iterator<Item = style::Styled<&'a str>> {
   gen move {
      let mut literal_start_offset = 0;

      for (offset, c) in s.char_indices() {
         let Some(escaped_) = escape(c)
            .is_first(offset == 0)
            .maybe_delimiter(delimiter)
            .call()
         else {
            continue;
         };

         yield s[literal_start_offset..offset].style(normal);
         literal_start_offset = offset;

         yield escaped_.style(escaped);
         literal_start_offset += c.len_utf8();
      }

      yield s[literal_start_offset..s.len()].style(normal);
   }
}

macro_rules! token {
   (
      #[from($kind:ident)]
      $(#[$attribute:meta])*
      struct $name:ident;
   ) => {
      $(#[$attribute])*
      #[derive(Deref, Debug, Clone, PartialEq, Eq, Hash)]
      #[repr(transparent)]
      pub struct $name(red::Token);

      impl fmt::Display for $name {
         fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
            (**self).fmt(writer)
         }
      }

      impl<'a> TryFrom<&'a red::Token> for &'a $name {
         type Error = ();

         fn try_from(token: &'a red::Token) -> Result<Self, ()> {
            if token.kind() != $kind {
               return Err(());
            }

            // SAFETY: token is &red::Token and we are casting it to &$name.
            // $name holds red::Token with #[repr(transparent)], so the layout
            // is the exact same for &red::Token and &$name.
            Ok(unsafe { &*ptr::from_ref(token).cast::<$name>() })
         }
      }

      impl TryFrom<red::Token> for $name {
         type Error = ();

         fn try_from(token: red::Token) -> Result<Self, ()> {
            if token.kind() != $kind {
               return Err(());
            }

            Ok(Self(token))
         }
      }
   };
}

// SPACE

token! {
   #[from(TOKEN_SPACE)]
   /// Space. Anything that matches [`char::is_whitespace`].
   struct Space;
}

impl Space {
   /// Returns the amount of lines this space.
   #[must_use]
   pub fn line_count(&self) -> usize {
      self.text().bytes().filter(|&c| c == b'\n').count() + 1
   }
}

// COMMENT

token! {
   #[from(TOKEN_COMMENT)]
   /// A multiline or singleline comment.
   struct Comment;
}

impl Comment {
   const START_HASHTAG_LEN: usize = '#'.len_utf8();

   /// Returns the starting delimiter of this comment.
   #[must_use]
   pub fn start_delimiter(&self) -> &str {
      let text = self.text();

      let content_start_index = text
         .bytes()
         .skip(Self::START_HASHTAG_LEN)
         .take_while(|&c| c == b'=')
         .count()
         + Self::START_HASHTAG_LEN;

      &text[..content_start_index]
   }

   /// Whether this comment has the capability to span multiple
   /// lines.
   #[must_use]
   pub fn is_multiline(&self) -> bool {
      self
         .text()
         .as_bytes()
         .get(Self::START_HASHTAG_LEN)
         .copied()
         .is_some_and(|c| c == b'=')
   }
}

// IDENTIFIER

token! {
   #[from(TOKEN_IDENTIFIER)]
   /// A plain identifier.
   struct Identifier;
}

// CONTENT

token! {
   #[from(TOKEN_CONTENT)]
   struct Content;
}

// INTEGER

token! {
   #[from(TOKEN_INTEGER)]
   /// An integer.
   struct Integer;
}

impl Integer {
   /// Returns the value of this integer, after resolving binary,
   /// octadecimal and hexadecimal notation if it exists.
   #[rustfmt::skip]
   pub fn value(&self) -> Result<num::BigInt, num_bigint::ParseBigIntError> {
      let text = self.text();

      match text.as_bytes().get(1).copied() {
         // Remove leading `_` because that makes num_bigint's parser see the literal as `_BEEF` when we have `0x_BEEF`.
         Some(b'b' | b'B') => num::BigInt::from_str_radix(text["0b".len()..].trim_start_matches('_'), 2),
         Some(b'o' | b'O') => num::BigInt::from_str_radix(text["0o".len()..].trim_start_matches('_'), 8),
         Some(b'x' | b'X') => num::BigInt::from_str_radix(text["0x".len()..].trim_start_matches('_'), 16),
         _ => num::BigInt::from_str_radix(text, 10),
      }
   }
}

// FLOAT

token! {
   #[from(TOKEN_FLOAT)]
   /// A float.
   struct Float;
}

impl Float {
   /// Returns the value of the float by parsing the underlying slice.
   pub fn value(&self) -> Result<f64, num_traits::ParseFloatError> {
      let text = self.text();

      match text.as_bytes().get(1).copied() {
         // Remove leading `_` because that makes num_bigint's parser see the literal as `_BEEF`
         // when we have `0x_BE.EF`.
         Some(b'b' | b'B') => f64::from_str_radix(text["0b".len()..].trim_start_matches('_'), 2),
         Some(b'o' | b'O') => f64::from_str_radix(text["0o".len()..].trim_start_matches('_'), 8),
         Some(b'x' | b'X') => f64::from_str_radix(text["0x".len()..].trim_start_matches('_'), 16),
         _ => f64::from_str_radix(text, 10),
      }
   }
}

//! Typed [`Token`] definitions.
//!
//! [`Token`]: crate::Token
use std::{
   fmt,
   ptr,
};

use derive_more::Deref;
use num::Num as _;

use crate::{
   Kind::*,
   red,
};

const EXPECT_INTEGER_VALID: &str = "integer token must be valid";
const EXPECT_FLOAT_VALID: &str = "float token must be valid";

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
            (&**self).fmt(writer)
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

// WHITESPACE

token! {
   #[from(TOKEN_WHITESPACE)]
   /// Whitespace. Anything that matches [`char::is_whitespace`].
   struct Whitespace;
}

impl Whitespace {
   /// Returns the amount of lines this whitespace.
   #[must_use]
   pub fn newline_count(&self) -> usize {
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

   /// Whether if this comment has the capability to span multiple
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
   #[must_use]
   #[rustfmt::skip]
   pub fn value(&self) -> num::BigInt {
      let text = self.text();

      match text.as_bytes().get(1).copied() {
         Some(b'b' | b'B') => num::BigInt::from_str_radix(text.get(2..).expect(EXPECT_INTEGER_VALID), 2),
         Some(b'o' | b'O') => num::BigInt::from_str_radix(text.get(2..).expect(EXPECT_INTEGER_VALID), 8),
         Some(b'x' | b'X') => num::BigInt::from_str_radix(text.get(2..).expect(EXPECT_INTEGER_VALID), 16),
         _ => num::BigInt::from_str_radix(text, 10),
      }
      .expect(EXPECT_INTEGER_VALID)
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
   #[must_use]
   pub fn value(&self) -> f64 {
      let text = self.text();

      match text.as_bytes().get(1).copied() {
         Some(b'b' | b'B') => f64::from_str_radix(text.get(2..).expect(EXPECT_FLOAT_VALID), 2),
         Some(b'o' | b'O') => f64::from_str_radix(text.get(2..).expect(EXPECT_FLOAT_VALID), 8),
         Some(b'x' | b'X') => f64::from_str_radix(text.get(2..).expect(EXPECT_FLOAT_VALID), 16),
         _ => f64::from_str_radix(text, 10),
      }
      .expect(EXPECT_FLOAT_VALID)
   }
}

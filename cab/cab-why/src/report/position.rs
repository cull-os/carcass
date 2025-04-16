use std::{
   ops,
   sync::OnceLock,
};

use smallvec::SmallVec;

use crate::{
   Size,
   Span,
   width,
};

/// A position in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
   /// The line number. One indexed.
   pub line:   u32,
   /// The column. One indexed, but zero means we are at the newline.
   ///
   /// The column is not a raw byte index, but a char index.
   ///
   /// The newline in the following string is at line 2, column 0: `"foo\nbar"`
   pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionStr<'a> {
   content:  &'a str,
   newlines: OnceLock<SmallVec<Size, 16>>,
}

impl<'a> ops::Deref for PositionStr<'a> {
   type Target = &'a str;

   fn deref(&self) -> &Self::Target {
      &self.content
   }
}

impl<'a> PositionStr<'a> {
   pub fn new(content: &'a str) -> Self {
      Self {
         content,
         newlines: OnceLock::new(),
      }
   }

   pub fn position(&self, offset: Size) -> Position {
      let newlines = self.newlines.get_or_init(|| {
         self
            .content
            .bytes()
            .enumerate()
            .filter_map(|(index, c)| (c == b'\n').then_some(Size::new(index)))
            .collect()
      });

      match newlines.binary_search(&offset) {
         Ok(line_index) => {
            Position {
               line:   line_index as u32 + 1,
               column: 1,
            }
         },

         Err(line_index) => {
            let line_start = if line_index == 0 {
               0
            } else {
               *newlines[line_index - 1] + 1
            };

            Position {
               line:   line_index as u32 + 1,
               column: *width(&self.content[Span::std(line_start, offset)]),
            }
         },
      }
   }

   pub fn positions(&self, span: Span) -> (Position, Position) {
      (self.position(span.start), self.position(span.end))
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_position() {
      let source = PositionStr::new("foo\nbar");
      assert_eq!(&source[0..5], "foo\nb");
      assert_eq!(
         source.positions(Span::new(0u32, 5u32)),
         (
            Position {
               line:   1,
               column: 0,
            },
            Position {
               line:   2,
               column: 1,
            }
         )
      );

      let source = PositionStr::new("foo\næ");
      assert_eq!(&source[0..6], "foo\næ");
      assert_eq!(
         source.positions(Span::new(0u32, 6u32)),
         (
            Position {
               line:   1,
               column: 0,
            },
            Position {
               line:   2,
               column: 1,
            }
         )
      );

      let source = PositionStr::new("foo\næb");
      assert_eq!(&source[0..6], "foo\næ");
      assert_eq!(
         source.positions(Span::new(0u32, 6u32)),
         (
            Position {
               line:   1,
               column: 0,
            },
            Position {
               line:   2,
               column: 1,
            }
         )
      );
      assert_eq!(
         source.positions(Span::new(0u32, 7u32)),
         (
            Position {
               line:   1,
               column: 0,
            },
            Position {
               line:   2,
               column: 2,
            }
         )
      );
   }
}

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
         Ok(line_index) | Err(line_index) => {
            let line_start = if line_index == 0 {
               0
            } else {
               *newlines[line_index - 1] + 1
            };

            Position {
               line:   line_index as u32 + 1,
               column: *width(&self.content[Span::std(line_start, offset)]) + 1,
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
      let mut source;

      macro_rules! assert_span {
         (
            $range:expr =>
            $slice:literal,($start_line:literal : $start_column:literal),($end_line:literal : $end_column:literal)
         ) => {
            assert_eq!(&source[$range], $slice);

            let (start, end) = source.positions(Span::new($range.start as u32, $range.end as u32));

            assert_eq!(start, Position {
               line:   $start_line,
               column: $start_column,
            });
            assert_eq!(end, Position {
               line:   $end_line,
               column: $end_column,
            });
         };
      }

      source = PositionStr::new("foo\nbar");
      assert_span!(0..5 => "foo\nb", (1:1), (2:2));
      assert_span!(0..1 => "f", (1:1), (1:2));

      source = PositionStr::new("foo\næ");
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));

      source = PositionStr::new("foo\næb");
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));
      assert_span!(0..2 => "fo", (1:1), (1:3));
      assert_span!(0..4 => "foo\n", (1:1), (2:1));
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));
      assert_span!(0..7 => "foo\næb", (1:1), (2:3));
      assert_span!(3..7 => "\næb", (2:1), (2:3));
   }
}

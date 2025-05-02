use std::{
   ops,
   sync::OnceLock,
};

use cab_format::width;
use cab_span::{
   Size,
   Span,
};
use smallvec::SmallVec;

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
   #[must_use]
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

      let line_index = newlines.partition_point(|&line_offset| offset > line_offset);

      let line_start = if line_index == 0 {
         0
      } else {
         *newlines[line_index - 1] + 1
      };

      Position {
         line:   u32::try_from(line_index).expect("line index must fit in u32") + 1,
         column: u32::try_from(width(&self.content[Span::std(line_start, offset)]))
            .expect("column must fit in u32")
            + 1,
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
   fn position() {
      let mut source;

      macro_rules! assert_span {
         (
            $range:expr =>
            $slice:literal,($start_line:literal : $start_column:literal),($end_line:literal : $end_column:literal)
         ) => {
            let range: ops::Range<usize> = $range;

            assert_eq!(&source[range.clone()], $slice);

            let (start, end) = source.positions(Span::new(range.start, range.end));

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
      assert_span!(3..7 => "\næb", (1:4), (2:3));
      assert_span!(4..7 => "æb", (2:1), (2:3));
   }
}

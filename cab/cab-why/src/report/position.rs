use crate::Span;

/// A position in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
   /// The line number. One indexed.
   pub line: u32,
   /// The column. One indexed, but zero means we are at the newline.
   ///
   /// The column is not a raw byte index, but a char index.
   ///
   /// The newline in the following string is at line 2, column 0: `"foo\nbar"`
   pub column: u32,
}

impl Position {
   /// Calculates the start and end position of the span in the given source.
   pub fn of(span: Span, source: &str) -> (Position, Position) {
      let range: std::ops::Range<usize> = span.into();

      let mut line = 1;
      let mut column = 1;

      let mut start = Position { line, column };
      let mut end = Position { line, column };

      let mut index = 0;

      for c in source.chars() {
         index += c.len_utf8();

         if c == '\n' {
            line += 1;
            column = 0;
         } else {
            column += 1;
         }

         if index == range.start {
            start.line = line;
            start.column = column;
         }

         if index >= range.end {
            end.line = line;
            end.column = column;

            break;
         }
      }

      (start, end)
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn test_position() {
      let source = "foo\nbar";
      assert_eq!(&source[0..5], "foo\nb");
      assert_eq!(
         Position::of(Span::new(0u32, 5u32), source),
         (
            Position { line: 1, column: 1 },
            Position { line: 2, column: 1 }
         )
      );

      let source = "foo\næ";
      assert_eq!(&source[0..6], "foo\næ");
      assert_eq!(
         Position::of(Span::new(0u32, 6u32), source),
         (
            Position { line: 1, column: 1 },
            Position { line: 2, column: 1 }
         )
      );

      let source = "foo\næb";
      assert_eq!(&source[0..6], "foo\næ");
      assert_eq!(
         Position::of(Span::new(0u32, 5u32), source),
         (
            Position { line: 1, column: 1 },
            Position { line: 2, column: 1 }
         )
      );
      assert_eq!(
         Position::of(Span::new(0u32, 6u32), source),
         (
            Position { line: 1, column: 1 },
            Position { line: 2, column: 1 }
         )
      );
      assert_eq!(
         Position::of(Span::new(0u32, 7u32), source),
         (
            Position { line: 1, column: 1 },
            Position { line: 2, column: 2 }
         )
      );
   }
}

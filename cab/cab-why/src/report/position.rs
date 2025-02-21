use std::num;

use unicode_segmentation::UnicodeSegmentation as _;

use crate::Span;

/// A position in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    /// The line number. One indexed.
    pub line: num::NonZeroU32,
    /// The column. One indexed, but zero means we are at the newline.
    ///
    /// The column is not a raw byte index, but a grapheme index.
    ///
    /// The newline in the following string is at line 2, column 0: `"foo\nbar"`
    pub column: u32,
}

impl Position {
    /// Calculates the start and end position of the span in the given source.
    pub fn of(span: Span, source: &str) -> (Position, Position) {
        let range: std::ops::Range<usize> = span.into();

        let mut line = num::NonZeroU32::MIN;
        let mut column = 1;

        let mut start = Position { line, column };
        let mut end = Position { line, column };

        for (index, c) in source.grapheme_indices(true) {
            if index > range.end {
                break;
            }

            if c == "\n" {
                line = line.saturating_add(1);
                column = 0;
            } else {
                column += 1;
            }

            if index + 1 == range.start {
                start.line = line;
                start.column = column;
            }

            if index + 1 == range.end {
                end.line = line;
                end.column = column;
            }
        }

        (start, end)
    }
}

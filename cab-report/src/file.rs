use std::num;

use cab_text::Span;
use unicode_segmentation::UnicodeSegmentation as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: num::NonZeroU32,
    pub column: u32,
}

impl Position {
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

            if index + 1 == range.start {
                start.line = line;
                start.column = column;
            }

            if c == "\n" {
                line = line.saturating_add(1);
                column = 0;
            } else {
                column += 1;
            }

            if index + 1 == range.end {
                end.line = line;
                end.column = column;
            }
        }

        (start, end)
    }
}

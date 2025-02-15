use std::num;

use cab_text::Span;

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: num::NonZeroU32,
    pub column: u32,
}

#[derive(Debug)]
pub struct File<'a> {
    // TODO: Use a real island type.
    pub island: CowStr,
    pub path: CowStr,
    pub source: borrow::Cow<'a, str>,
}

impl File<'_> {
    pub fn position_of(&self, span: Span) -> (Position, Position) {
        let range: std::ops::Range<usize> = span.into();

        let mut line = num::NonZeroU32::MIN;
        let mut column = 1;

        let mut start = Position { line, column };
        let mut end = Position { line, column };

        for (index, c) in self.source.char_indices() {
            if index > range.end {
                break;
            }

            if index == range.start {
                start.line = line;
                start.column = column;
            }

            if c == '\n' {
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

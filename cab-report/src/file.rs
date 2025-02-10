use std::{
    num::NonZeroUsize,
    ops,
};

use crate::*;

#[derive(Debug)]
pub struct File<'a> {
    // TODO: Use a real island type.
    pub island: CowStr<'a>,
    pub path: CowStr<'a>,
    pub source: CowStr<'a>,
}

impl File<'_> {
    pub fn position(&self, range: &ops::Range<usize>) -> (Position, Position) {
        let mut line = NonZeroUsize::MIN;
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: NonZeroUsize,
    pub column: usize,
}

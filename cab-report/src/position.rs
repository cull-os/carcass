use std::ops;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn of(range: &ops::Range<usize>, source: &str) -> (Self, Self) {
        let mut start = Self { line: 1, column: 1 };
        let mut end = Self { line: 1, column: 1 };

        for (index, c) in source.char_indices() {
            if index > range.end {
                break;
            }

            if index <= range.start {
                if c == '\n' {
                    start.line += 1;
                    start.column = 1;
                } else {
                    start.column += 1;
                }
            }

            if index <= range.end {
                if c == '\n' {
                    end.line += 1;
                    end.column = 1;
                } else {
                    end.column += 1;
                }
            }
        }

        (start, end)
    }
}

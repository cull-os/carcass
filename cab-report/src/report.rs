use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt::{
        self,
        Write as _,
    },
    ops,
};

use yansi::Paint;

use crate::*;

pub struct Report<'a> {
    title: CowStr<'a>,
    level: log::Level,
    labels: Vec<Label<'a>>,
    tips: Vec<Tip<'a>>,
}

impl<'a> Report<'a> {
    pub fn new(title: impl Into<CowStr<'a>>, level: log::Level) -> Self {
        Self {
            title: title.into(),
            level,
            labels: Vec::new(),
            tips: Vec::new(),
        }
    }

    pub fn error(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(title, log::Level::Error)
    }

    pub fn warn(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(title, log::Level::Warn)
    }

    pub fn info(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(title, log::Level::Info)
    }

    pub fn push_label(&mut self, label: Label<'a>) {
        self.labels.push(label)
    }

    pub fn label(mut self, label: Label<'a>) -> Self {
        self.push_label(label);
        self
    }

    pub fn push_primary(&mut self, range: ops::Range<usize>, text: impl Into<CowStr<'a>>) {
        self.push_label(Label::primary(range, text));
    }

    pub fn primary(mut self, range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        self.push_primary(range, text);
        self
    }

    pub fn push_secondary(&mut self, range: ops::Range<usize>, text: impl Into<CowStr<'a>>) {
        self.push_label(Label::secondary(range, text));
    }

    pub fn secondary(mut self, range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        self.push_secondary(range, text);
        self
    }

    pub fn push_tip(&mut self, tip: Tip<'a>) {
        self.tips.push(tip)
    }

    pub fn tip(mut self, tip: Tip<'a>) -> Self {
        self.push_tip(tip);
        self
    }

    pub fn push_note(&mut self, text: impl Into<CowStr<'a>>) {
        self.push_tip(Tip::note(text));
    }

    pub fn note(self, text: impl Into<CowStr<'a>>) -> Self {
        self.tip(Tip::note(text))
    }

    pub fn push_help(&mut self, text: impl Into<CowStr<'a>>) {
        self.push_tip(Tip::help(text));
    }

    pub fn help(self, text: impl Into<CowStr<'a>>) -> Self {
        self.tip(Tip::help(text))
    }

    pub fn log_with(&'a self, file: &'a File<'a>) {
        let with_file = ReportDisplay { report: self, file };
        log::log!(self.level, "{with_file}");
    }
}

struct ReportDisplay<'a> {
    report: &'a Report<'a>,
    file: &'a File<'a>,
}

fn number_width(number: usize) -> usize {
    if number == 0 {
        1
    } else {
        (number as f64).log10() as usize + 1
    }
}

fn shrink_to_line_boundaries(source: &str, mut range: ops::Range<usize>) -> ops::Range<usize> {
    while range.start < range.end {
        if source[range.start..].starts_with('\n') {
            range.start += 1;
            break;
        }
        range.start += 1;
    }

    while range.end > range.start {
        if source[..range.end].ends_with('\n') {
            range.end -= 1;
            break;
        }
        range.end -= 1;
    }

    range
}
fn extend_to_line_boundaries(source: &str, mut range: ops::Range<usize>) -> ops::Range<usize> {
    while range.start > 0 && !source[range.start - 1..].starts_with('\n') {
        range.start -= 1;
    }

    while range.end < source.len() && !source[range.end..].starts_with('\n') {
        range.end += 1;
    }

    range
}

impl fmt::Display for ReportDisplay<'_> {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        const LEFT_TO_RIGHT: char = '━';
        const BOTTOM_TO_RIGHT: char = '┏';
        const TOP_TO_RIGHT: char = '┗';
        const TOP_TO_RIGHT_AND_BOTTOM: char = '┣';
        const TOP_TO_BOTTOM: char = '┃';
        const DOT: char = '·';

        let gutter_style_number = yansi::Style::new().blue().bold();
        let gutter_style_line = yansi::Style::new().blue();

        let gutter_style_header_path = yansi::Style::new().green();
        let gutter_style_header_position = yansi::Style::new().blue();

        let Report {
            title,
            labels,
            tips,
            ..
        } = self.report;

        let file = self.file;

        // TITLE
        writeln!(writer, "{title}", title = title.bright_white().bold())?;

        let mut labels: Vec<_> = labels
            .clone()
            .into_iter()
            .map(|label| (Position::from(&label.range, file), label))
            .collect();

        labels.sort_by_key(|(.., label)| label.range.start);

        let line_number_max_width = labels
            .iter()
            .map(|((.., end), ..)| end.line)
            .max()
            .map(number_width)
            .unwrap_or(0);

        let line_number = RefCell::new(None::<usize>);

        let mut _line_number_last = None;
        indent!(writer, line_number_max_width + 3, with: |writer: &mut dyn fmt::Write| {
            let Some(line_number) = *line_number.borrow() else { return Ok(0) };

            if _line_number_last == Some(line_number) {
                let dot_width = number_width(line_number);
                write!(writer, "{:>space_width$}", "", space_width = line_number_max_width - dot_width)?;

                gutter_style_number.fmt_prefix(writer)?;
                for _ in 0..dot_width {
                    write!(writer, "{DOT}")?;
                }
                gutter_style_number.fmt_suffix(writer)?;
            } else {
                write!(writer, "{line_number:>line_number_max_width$}", line_number = line_number.paint(gutter_style_number))?;
            }

            write!(writer, " {separator} ", separator = TOP_TO_BOTTOM.paint(gutter_style_line))?;

            _line_number_last = Some(line_number);
            Ok(line_number_max_width + 3)
        });

        // ┏━━━ <island>/path:{line}:{column}
        {
            // Dedent that separator and space as we are going to align.
            dedent!(writer, 2);

            if let Some(((start, ..), ..)) = labels.first() {
                indent!(writer, header: const_str::concat!(BOTTOM_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT).paint(gutter_style_line));

                let File { island, path, .. } = file;
                gutter_style_header_path.fmt_prefix(writer)?;
                write!(writer, "{island}{path}")?;
                gutter_style_header_path.fmt_suffix(writer)?;

                let line = start.line.paint(gutter_style_header_position);
                let column = start.column.paint(gutter_style_header_position);
                writeln!(writer, ":{line}:{column}")?;
            }
        }

        'code: {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            struct LabelId(usize);

            #[derive(Debug, Clone)]
            struct LineData<'a> {
                number: usize,
                content: &'a str,
                styles: Vec<(ops::Range<usize>, LabelLevel)>,
                prefix: VecDeque<(LabelId, yansi::Painted<char>)>,
                finish: Vec<(LabelId, &'a Label<'a>)>,
            }

            let mut lines: Vec<LineData> = Vec::new();

            for (id, ((start, end), label)) in labels
                .iter()
                .enumerate()
                .map(|(index, item)| (LabelId(index), item))
            {
                let shrinked = shrink_to_line_boundaries(&file.source, label.range());
                let normal = label.range();
                let full = extend_to_line_boundaries(&file.source, label.range());

                // TODO: .saturating_sub isn't the right thing to do...
                let start_colored = {
                    let base = full.start;
                    normal.start - base..(shrinked.start - base).saturating_sub(1)
                };

                let end_colored = {
                    let base = shrinked.end;
                    shrinked.end - base..(normal.end - base).saturating_sub(1)
                };

                for (line_number, line) in (start.line..).zip(file.source[full].split('\n')) {
                    let item = match lines.iter_mut().find(|line| line.number == line_number) {
                        Some(item) => item,
                        None => {
                            lines.push(LineData {
                                number: line_number,
                                content: line,
                                styles: Vec::new(),
                                prefix: VecDeque::new(),
                                finish: Vec::new(),
                            });

                            lines.last_mut().unwrap()
                        },
                    };

                    if start.line != end.line {
                        let mut prefix = if line_number == start.line {
                            BOTTOM_TO_RIGHT
                        } else {
                            TOP_TO_BOTTOM
                        }
                        .new();

                        prefix.style = label.style();

                        item.prefix.push_front((id, prefix));
                    }

                    if line_number == end.line {
                        item.finish.push((id, label));
                    }

                    if start.line == end.line {
                        assert_eq!(start_colored, end_colored);
                        item.styles.push((start_colored.clone(), label.level));
                    } else if line_number == start.line {
                        item.styles.push((start_colored.clone(), label.level));
                    } else if line_number == end.line {
                        item.styles.push((end_colored.clone(), label.level));
                    } else {
                        item.styles.push((0..line.width(), label.level));
                    }
                }
            }

            let Some(prefix_width) = lines.iter().map(|line| line.prefix.len()).max() else {
                break 'code;
            };

            let current_line = RefCell::new(lines.first().unwrap());

            // +1 for a space at the start.
            indent!(writer, prefix_width + 1, with: |writer: &mut dyn fmt::Write| {
                let line = current_line.borrow_mut();

                let space_width = prefix_width - line.prefix.len();
                write!(writer, "{:>space_width$}", "")?;

                for (.., prefix) in line.prefix.iter().rev() {
                    write!(writer, "{prefix}")?;
                }
                Ok(prefix_width)
            });

            for line in &lines {
                *line_number.borrow_mut() = Some(line.number);
                *current_line.borrow_mut() = line;

                let mut styles = line.styles.clone();
                write_wrapped(writer, resolve_style(line.content, styles.as_mut_slice()))?;
                writeln!(writer)?;

                // for (id, label) in &line.finish {
                // TODO
                // }
            }
        }

        // = help: foo bar
        {
            *line_number.borrow_mut() = None;

            // Dedent that separator as we are going to align.
            dedent!(
                writer,
                if line_number_max_width == 0 {
                    line_number_max_width + 3
                } else {
                    line_number_max_width + 1
                }
            );

            for tip in tips {
                indent!(writer, header: "=".paint(gutter_style_line));
                write!(writer, "{tip}")?;
            }
        }

        Ok(())
    }
}

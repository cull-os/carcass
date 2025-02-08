use std::{
    cell::RefCell,
    fmt::{
        self,
        Write as _,
    },
    iter,
    num::NonZeroUsize,
    ops,
};

use smallvec::SmallVec;
use yansi::Paint;

use crate::*;

pub struct Report<'a> {
    title: CowStr<'a>,
    level: log::Level,
    labels: SmallVec<Label<'a>, 2>,
    tips: SmallVec<Tip<'a>, 2>,
}

impl<'a> Report<'a> {
    pub fn new(title: impl Into<CowStr<'a>>, level: log::Level) -> Self {
        Self {
            title: title.into(),
            level,
            labels: SmallVec::new(),
            tips: SmallVec::new(),
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
        const HORIZONTAL: char = '━';
        const LEFT_TO_BOTTOM: char = '┓';
        const BOTTOM_TO_RIGHT: char = '┏';
        const TOP_TO_RIGHT: char = '┗';
        const VERTICAL: char = '┃';
        const DOT: char = '·';

        const GUTTER_STYLE_NUMBER: yansi::Style = yansi::Style::new().blue().bold();
        const GUTTER_STYLE_LINE: yansi::Style = yansi::Style::new().blue();

        const GUTTER_STYLE_HEADER_PATH: yansi::Style = yansi::Style::new().green();
        const GUTTER_STYLE_HEADER_POSITION: yansi::Style = yansi::Style::new().blue();

        let Report {
            title,
            labels,
            tips,
            ..
        } = self.report;

        let file = self.file;

        // TITLE
        writeln!(writer, "{title}", title = title.bright_white().bold())?;

        let mut labels: SmallVec<_, 2> = labels
            .clone()
            .into_iter()
            .map(|label| (Position::from(&label.range, file), label))
            .collect();

        labels.sort_by_key(|(_, label)| label.range.start);

        let line_number_max_width = labels
            .iter()
            .map(|((_, end), _)| end.line)
            .max()
            .map(number_width)
            .unwrap_or(0);

        let line_number = RefCell::new(None::<usize>);

        // INDENT: Line numbers and gutter "123 | ".
        let mut line_number_last = None;
        indent!(writer, line_number_max_width + 3, with: |writer: &mut dyn fmt::Write| {
            let Some(line_number) = *line_number.borrow() else { return Ok(0) };

            if line_number_last == Some(line_number) {
                let dot_width = number_width(line_number);
                write!(writer, "{:>space_width$}", "", space_width = line_number_max_width - dot_width)?;

                GUTTER_STYLE_NUMBER.fmt_prefix(writer)?;
                for _ in 0..dot_width {
                    write!(writer, "{DOT}")?;
                }
                GUTTER_STYLE_NUMBER.fmt_suffix(writer)?;
            } else {
                write!(writer, "{line_number:>line_number_max_width$}", line_number = line_number.paint(GUTTER_STYLE_NUMBER))?;
            }

            write!(writer, " {separator} ", separator = VERTICAL.paint(GUTTER_STYLE_LINE))?;

            line_number_last = Some(line_number);
            Ok(line_number_max_width + 3)
        });

        {
            // Dedent that separator and space as we are going to align.
            dedent!(writer, 2);

            if let Some(((start, _), _)) = labels.first() {
                // INDENT: "┏━━━ ".
                indent!(writer, header: const_str::concat!(BOTTOM_TO_RIGHT, HORIZONTAL, HORIZONTAL, HORIZONTAL).paint(GUTTER_STYLE_LINE));

                let File { island, path, .. } = file;
                GUTTER_STYLE_HEADER_PATH.fmt_prefix(writer)?;
                write!(writer, "{island}{path}")?;
                GUTTER_STYLE_HEADER_PATH.fmt_suffix(writer)?;

                let line = start.line.paint(GUTTER_STYLE_HEADER_POSITION);
                let column = start.column.paint(GUTTER_STYLE_HEADER_POSITION);
                writeln!(writer, ":{line}:{column}")?;
            }
        }

        // TODO:
        // - Right facing corners pierce though verticals.
        // - Single line error labels.
        // - Primary/secondary label colors depend on report level.
        'display_lines: {
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            struct StrikeId(NonZeroUsize);

            #[derive(Debug, Clone)]
            enum Strike<'a> {
                Start(yansi::Style),
                Continue(yansi::Style),
                End(ops::Range<usize>, &'a Label<'a>),
            }

            impl Strike<'_> {
                fn symbol(&self) -> yansi::Painted<&'static char> {
                    match self {
                        Self::Start(style) => BOTTOM_TO_RIGHT.paint(*style),
                        Self::Continue(style) => VERTICAL.paint(*style),
                        Self::End(_, label) => TOP_TO_RIGHT.paint(label.style()),
                    }
                }

                fn right_symbol(&self) -> Option<yansi::Painted<&'static char>> {
                    match self {
                        Strike::Start(style) => Some(HORIZONTAL.paint(*style)),
                        Strike::Continue(_) | Strike::End(..) => None,
                    }
                }
            }

            #[derive(Debug, Clone)]
            struct Line<'a> {
                number: usize,
                content: &'a str,
                styles: SmallVec<(ops::Range<usize>, LabelLevel), 4>,
                strikes: SmallVec<(StrikeId, Strike<'a>), 3>,
            }

            let mut lines: SmallVec<Line, 2> = SmallVec::new();

            for (id, ((start, end), label)) in labels
                .iter()
                .enumerate()
                .map(|(index, item)| (StrikeId(NonZeroUsize::MIN.saturating_add(index)), item))
            {
                let multiline = start.line != end.line;

                let shrinked = shrink_to_line_boundaries(&file.source, label.range());
                let normal = label.range();
                let extended = extend_to_line_boundaries(&file.source, label.range());

                let start_colored = {
                    let base = extended.start;
                    normal.start - base..(shrinked.start - base).saturating_sub(1)
                };

                let end_colored = {
                    let base = shrinked.end;
                    shrinked.end - base..(normal.end - base)
                };

                for (line_number, line) in (start.line..).zip(file.source[extended].split('\n')) {
                    let item = match lines.iter_mut().find(|line| line.number == line_number) {
                        Some(item) => item,

                        None => {
                            lines.push(Line {
                                number: line_number,
                                content: line,
                                styles: SmallVec::new(),
                                strikes: SmallVec::new(),
                            });

                            lines.last_mut().unwrap()
                        },
                    };

                    if multiline && line_number == start.line {
                        item.strikes.push((id, Strike::Start(label.style())));
                    }

                    if multiline
                        && (line_number == end.line
                            || (end.column == 0 && line_number == end.line - 1))
                    {
                        item.strikes
                            .push((id, Strike::End(end_colored.clone(), label)));
                    }

                    #[allow(clippy::if_same_then_else)]
                    if !multiline {
                        item.styles.push((start_colored.clone(), label.level));
                    } else if line_number == start.line {
                        item.styles.push((start_colored.clone(), label.level));
                    } else if line_number == end.line {
                        item.styles.push((end_colored.clone(), label.level));
                    } else {
                        item.styles.push((0..line.len(), label.level));
                    }
                }
            }

            let Some(strike_prefix_width) = lines.iter().map(|line| line.strikes.len()).max()
            else {
                break 'display_lines;
            };

            let mut strikes = SmallVec::<Option<(StrikeId, Strike)>, 3>::from_iter(iter::repeat_n(
                None,
                strike_prefix_width,
            ));
            let strikes_patch = RefCell::new(SmallVec::<(StrikeId, Strike), 3>::new());

            // INDENT: "<strikes-prefix> ".
            indent!(writer, strike_prefix_width + 1, with: |writer: &mut dyn fmt::Write| {
                dbg!(&strikes);
                // We are at the line after the line where the ends with labels were.
                // We haven't patched the strikes yet so they point to the last line.
                //
                // No .rev() as we want to display labels closest to the right first
                // to not overlap anything.
                for index in 0..strikes.len() {
                    let Some((_, strike @ Strike::End(range, label))) = &strikes[index].clone() else { continue };

                    // INDENT: "<prefix-symbols><top-to-right><horizontal><left-to-bottom> " then
                    // INDENT: "<prefix-symbols>                          <top--to-bottom> "
                    let top_to_right_width = 1;
                    let prefix_symbols_width = (strike_prefix_width - index).saturating_sub(top_to_right_width);
                    let horizontal_width = range.end.min(20);
                    let left_to_bottom_width = 1;

                    let mut wrote_first = false;
                    indent!(writer, prefix_symbols_width + top_to_right_width + horizontal_width + left_to_bottom_width + 1, with: |writer: &mut dyn fmt::Write| {
                        if !wrote_first {
                            // Part before the top-to-right.
                            for strike in strikes.iter().rev().take(prefix_symbols_width) {
                                let symbol = match strike {
                                    Some((_, Strike::End(_, label))) => Strike::Continue(label.style()).symbol(),

                                    Some((_, strike)) => strike.symbol(),

                                    None => (&' ').new(),
                                };

                                write!(writer, "{symbol}")?;
                            }

                            // Top-to-right.
                            write!(writer, "{symbol}", symbol = strike.symbol())?;

                            // Horizontal.
                            for _ in (0..).take(horizontal_width) {
                                write!(writer, "{symbol}", symbol = HORIZONTAL.paint(label.style()))?;
                            }

                            // Left-to-bottom.
                            write!(writer, "{symbol}", symbol = LEFT_TO_BOTTOM.paint(label.style()))?;

                            strikes[index] = None;
                            wrote_first = true;
                        } else {
                            for strike in strikes.iter().rev().take(prefix_symbols_width + top_to_right_width) {
                                let symbol = match strike {
                                    Some((_, Strike::End(_, label))) => Strike::Continue(label.style()).symbol(),

                                    Some((_, strike)) => strike.symbol(),

                                    None => (&' ').new(),
                                };

                                write!(writer, "{symbol}")?;
                            }

                            for _ in (0..).take(horizontal_width) {
                                write!(writer, " ")?;
                            }

                            write!(writer, "{symbol}", symbol = VERTICAL.paint(label.style()))?;
                        }

                        Ok(prefix_symbols_width + top_to_right_width + horizontal_width + left_to_bottom_width)
                    });

                    writeln!(writer, "{label}")?;
                }

                for (strike_id, strike) in strikes_patch.borrow_mut().drain(..) {
                    match strike {
                        Strike::Start(_) => {
                            let slot = strikes.iter_mut().find(|slot| slot.is_none()).unwrap();

                            *slot = Some((strike_id, strike));
                        }

                        Strike::End(..) => {
                            let slot = strikes.iter_mut().flatten().find(|(id, _)| *id == strike_id).unwrap();

                            slot.1 = strike;
                        }

                        _ => unreachable!(),
                    }
                }

                let mut previous_strike = None::<&Strike<'_>>;
                for strike_slot in strikes.iter_mut().rev() {
                    let Some((_, strike)) = strike_slot else {
                        if let Some(right_symbol) = previous_strike.as_ref().and_then(|s| s.right_symbol()) {
                            write!(writer, "{right_symbol}")?;
                        } else {
                            write!(writer, " ")?;
                        }
                        continue;
                    };

                    match strike {
                        Strike::Start(style) => {
                            let next = Strike::Continue(*style);
                            write!(writer, "{symbol}", symbol = strike.symbol())?;
                            *strike = next;
                        },

                        Strike::Continue(_) => {
                            if let Some(right_symbol) = previous_strike.and_then(|s| s.right_symbol()) {
                                write!(writer, "{right_symbol}")?;
                            } else {
                                write!(writer, "{symbol}", symbol = strike.symbol())?;
                            }
                        },

                        Strike::End(_, label) => {
                            let symbol = Strike::Continue(label.style()).symbol();
                            write!(writer, "{symbol}")?;
                        }
                    }

                    previous_strike = Some(strike);
                }

                Ok(strike_prefix_width)
            });

            for line in lines {
                *line_number.borrow_mut() = Some(line.number);
                *strikes_patch.borrow_mut() = line.strikes;

                {
                    let mut styles = line.styles.clone();
                    write_wrapped(writer, resolve_style(line.content, styles.as_mut_slice()))?;
                    writeln!(writer)?;
                }
            }
            writeln!(writer, " ")?; // HACK
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
                // INDENT: "= ".
                indent!(writer, header: "=".paint(GUTTER_STYLE_LINE));
                write!(writer, "{tip}")?;
            }
        }

        Ok(())
    }
}

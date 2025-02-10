use std::{
    cell::RefCell,
    fmt::{
        self,
        Write as _,
    },
    iter,
    ops,
};

use smallvec::SmallVec;
use yansi::Paint;

use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportSeverity {
    Note,
    Warn,
    Error,
    Bug,
}

impl ReportSeverity {
    pub fn styled(self) -> yansi::Painted<&'static str> {
        match self {
            ReportSeverity::Note => "note:",
            ReportSeverity::Warn => "warn:",
            ReportSeverity::Error => "error:",
            ReportSeverity::Bug => "bug:",
        }
        .paint(LabelSeverity::Primary.style_in(self))
        .bold()
    }
}

pub struct Report<'a> {
    severity: ReportSeverity,
    title: CowStr<'a>,
    labels: SmallVec<Label<'a>, 2>,
    points: SmallVec<Point<'a>, 2>,
}

impl<'a> Report<'a> {
    pub fn new(severity: ReportSeverity, title: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: title.into(),
            severity,
            labels: SmallVec::new(),
            points: SmallVec::new(),
        }
    }

    pub fn note(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(ReportSeverity::Note, title)
    }

    pub fn warning(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(ReportSeverity::Warn, title)
    }

    pub fn error(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(ReportSeverity::Error, title)
    }

    pub fn bug(title: impl Into<CowStr<'a>>) -> Self {
        Self::new(ReportSeverity::Bug, title)
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

    pub fn push_point(&mut self, point: Point<'a>) {
        self.points.push(point)
    }

    pub fn point(mut self, point: Point<'a>) -> Self {
        self.push_point(point);
        self
    }

    pub fn push_tip(&mut self, text: impl Into<CowStr<'a>>) {
        self.push_point(Point::tip(text));
    }

    pub fn tip(self, text: impl Into<CowStr<'a>>) -> Self {
        self.point(Point::tip(text))
    }

    pub fn push_help(&mut self, text: impl Into<CowStr<'a>>) {
        self.push_point(Point::help(text));
    }

    pub fn help(self, text: impl Into<CowStr<'a>>) -> Self {
        self.point(Point::help(text))
    }

    pub fn with(self, file: &'a File<'a>) -> impl fmt::Display + 'a {
        ReportDisplay { report: self, file }
    }
}

struct ReportDisplay<'a> {
    report: Report<'a>,
    file: &'a File<'a>,
}

impl fmt::Display for ReportDisplay<'_> {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        const BOTTOM_TO_RIGHT: char = '┏';
        const TOP_TO_BOTTOM: char = '┃';
        const TOP_TO_RIGHT: char = '┗';
        const LEFT_TO_RIGHT: char = '━';
        const LEFT_TO_BOTTOM: char = '┓';
        const DOT: char = '·';

        const STYLE_GUTTER: yansi::Style = yansi::Style::new().blue();
        const STYLE_HEADER_PATH: yansi::Style = yansi::Style::new().green();
        const STYLE_HEADER_POSITION: yansi::Style = yansi::Style::new().blue();

        let Self { report, file } = self;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct StrikeId(usize);

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum StrikeStatus {
            Start,
            Continue,
            End,
        }

        impl StrikeStatus {
            fn symbol(self) -> char {
                match self {
                    Self::Start => BOTTOM_TO_RIGHT,
                    Self::Continue => TOP_TO_BOTTOM,
                    Self::End => TOP_TO_RIGHT,
                }
            }
        }

        type Strike = (StrikeId, StrikeStatus, LabelSeverity);

        let write_strike = |writer: &mut dyn fmt::Write, strike: &Option<Strike>| {
            if let Some((_, status, level)) = strike {
                write!(
                    writer,
                    "{strike}",
                    strike = status.symbol().paint(level.style_in(report.severity))
                )
            } else {
                write!(writer, " ")
            }
        };

        #[derive(Debug, Clone, PartialEq, Eq)]
        enum LabelRange {
            FromStart(ops::RangeTo<usize>),
            Inline(ops::Range<usize>),
        }

        impl LabelRange {
            fn end(&self) -> usize {
                match self {
                    LabelRange::FromStart(range) => range.end,
                    LabelRange::Inline(range) => range.end,
                }
            }
        }

        type LabelDynamic<'a> = (LabelRange, &'a CowStr<'a>, LabelSeverity);

        #[derive(Debug, Clone)]
        struct Line<'a> {
            number: usize,

            content: &'a str,
            styles: SmallVec<(ops::Range<usize>, LabelSeverity), 4>,

            strikes: SmallVec<Strike, 3>,
            labels: SmallVec<LabelDynamic<'a>, 2>,
        }

        let mut labels: SmallVec<_, 2> = report
            .labels
            .clone()
            .into_iter()
            .map(|label| (file.position(&label.range), label))
            .collect();

        labels.sort_by(|((a_start, _), a_label), ((b_start, _), b_label)| {
            if a_start != b_start {
                return a_start.cmp(b_start);
            }

            a_label.range.end.cmp(&b_label.range.end)
        });

        let mut lines: SmallVec<Line, 2> = SmallVec::new();
        let line_current = RefCell::new(None::<Line>);

        for (label_index, ((label_start, label_end), label)) in labels.iter().enumerate() {
            let label_is_multiline = label_start.line != label_end.line;

            let label_range_extended = extend_to_line_boundaries(&file.source, label.range());

            for (line_number, line) in (label_start.line.get()..)
                .zip(file.source[label_range_extended.clone()].split('\n'))
            {
                let line = match lines.iter_mut().find(|line| line.number == line_number) {
                    Some(item) => item,

                    None => {
                        lines.push(Line {
                            number: line_number,

                            content: line,
                            styles: SmallVec::new(),

                            strikes: SmallVec::new(),
                            labels: SmallVec::new(),
                        });

                        lines.last_mut().expect("we just pushed a line")
                    },
                };

                if !label_is_multiline {
                    let left = label_range_extended.start;

                    let start = label.range().start;
                    let end = label.range().end;

                    let range = start - left..end - left;

                    line.labels
                        .push((LabelRange::Inline(range.clone()), &label.text, label.level));

                    line.styles.push((range.clone(), label.level));

                    continue;
                }

                let strike_status = match line_number {
                    n if n == label_start.line.get() => StrikeStatus::Start,
                    n if n == label_end.line.get() => StrikeStatus::End,
                    _ => StrikeStatus::Continue,
                };

                line.strikes
                    .push((StrikeId(label_index), strike_status, label.level));

                let line_is_first = line_number == label_start.line.get();
                let line_is_last = line_number == label_end.line.get();

                match (line_is_first, line_is_last) {
                    (true, false) => {
                        let left = label_range_extended.start;

                        let start = label.range().start;
                        let end = file.source[start..]
                            .find('\n')
                            .map_or(file.source.len(), |index| start + index);

                        let range = start - left..end - left;

                        line.styles.push((range, label.level));
                    },

                    (false, false) => {
                        line.styles.push((0..line.content.len(), label.level));
                    },

                    (false, true) => {
                        let right = label_range_extended.end;

                        let start = 0;
                        let end = line.content.len() - (right - label.range().end);

                        let range = start..end;

                        line.labels.push((
                            LabelRange::FromStart(..range.end),
                            &label.text,
                            label.level,
                        ));

                        line.styles.push((range, label.level));
                    },

                    _ => unreachable!(),
                }
            }
        }

        let line_number_width = lines
            .iter()
            .map(|line| line.number)
            .max()
            .map(number_width)
            .unwrap_or(0);

        let strike_prefix_width = lines
            .iter()
            .map(|line| line.strikes.len())
            .max()
            .unwrap_or(0);

        let strike_prefix = RefCell::new(SmallVec::<_, 3>::from_iter(iter::repeat_n(
            None::<Strike>,
            strike_prefix_width,
        )));

        {
            // INDENT: "note: "
            indent!(writer, header: report.severity.styled());

            writeln!(
                writer,
                "{title}",
                title = report.title.bright_white().bold()
            )?;
        }

        // INDENT: "123 | "
        let mut line_number_previous = None;
        indent!(writer, line_number_width + 3, with: |writer: &mut dyn fmt::Write| {
            let line_current = line_current.borrow();
            let Some(line) = line_current.as_ref() else { return Ok(0) };

            STYLE_GUTTER.fmt_prefix(writer)?;

            if line_number_previous == Some(line.number) {
                let dot_width = number_width(line.number);

                write!(writer, "{:>space_width$}", "", space_width = line_number_width - dot_width)?;

                for _ in 0..dot_width {
                    write!(writer, "{DOT}")?;
                }
            } else {
                write!(writer, "{line_number:>line_number_width$}", line_number = line.number)?;
            }

            write!(writer, " {TOP_TO_BOTTOM} ")?;
            STYLE_GUTTER.fmt_suffix(writer)?;

            line_number_previous = Some(line.number);
            Ok(line_number_width + 3)
        });

        if let Some(line) = lines.first() {
            // DEDENT: "| "
            dedent!(writer, 2);

            // INDENT: "┏━━━ ".
            indent!(writer, header: const_str::concat!(BOTTOM_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT).paint(STYLE_GUTTER));

            STYLE_HEADER_PATH.fmt_prefix(writer)?;
            write!(
                writer,
                "{island}{path}",
                island = file.island,
                path = file.path
            )?;
            STYLE_HEADER_PATH.fmt_suffix(writer)?;

            let line_number = line.number.paint(STYLE_HEADER_POSITION);

            let column_number = line
                .styles
                .first()
                .expect("every line must have a non empty style")
                .0
                .start
                .paint(STYLE_HEADER_POSITION);

            writeln!(writer, ":{line_number}:{column_number}")?;
        }

        {
            // INDENT: "<strikes-prefix> "
            indent!(writer, strike_prefix_width + 1, with: |writer: &mut dyn fmt::Write| {
                let mut strike_override = None::<yansi::Painted<&char>>;

                for strike_slot in &*strike_prefix.borrow() {
                    let Some(mut strike @ (_, strike_status, strike_severity)) = *strike_slot else {
                        match strike_override {
                            Some(strike) => write!(writer, "{strike}")?,
                            None => write!(writer, " ")?,
                        }
                        continue;
                    };

                    match strike_status {
                        StrikeStatus::Start => {
                            write_strike(writer, &Some(strike))?;

                            strike_override = Some(LEFT_TO_RIGHT.paint(strike_severity.style_in(report.severity)));
                        },

                        StrikeStatus::Continue | StrikeStatus::End if let Some(strike) = strike_override => {
                            write!(writer, "{strike}")?;
                        }

                        StrikeStatus::Continue => {
                            write_strike(writer, &Some(strike))?;
                        }

                        StrikeStatus::End => {
                            strike.1 = StrikeStatus::Continue;
                            write_strike(writer, &Some(strike))?;
                        }
                    }
                }

                if let Some(strike) = strike_override {
                    write!(writer, "{strike}")?;

                    Ok(strike_prefix_width + 1)
                } else {
                    Ok(strike_prefix_width)
                }
            });

            for line in &mut lines {
                *line_current.borrow_mut() = Some(line.clone());

                {
                    let mut strike_prefix = strike_prefix.borrow_mut();

                    for strike_new @ (strike_id, ..) in &line.strikes {
                        match strike_prefix
                            .iter_mut()
                            .flatten()
                            .find(|(id, ..)| id == strike_id)
                        {
                            Some(strike) => *strike = *strike_new,

                            None => {
                                *strike_prefix
                                    .iter_mut()
                                    .find(|slot| slot.is_none())
                                    .unwrap() = Some(*strike_new);
                            },
                        }
                    }
                }

                {
                    write_wrapped(
                        writer,
                        resolve_style(line.content, &mut line.styles, report.severity),
                    )?;
                    writeln!(writer)?;

                    // DEDENT: "<strike-prefix> "
                    dedent!(writer);

                    for (label_range, label_text, label_severity) in line.labels.iter().rev() {
                        match label_range {
                            LabelRange::FromStart(label_range) => {
                                let &strike @ (strike_id, ..) = strike_prefix
                                    .borrow()
                                    .iter()
                                    .flatten()
                                    .rev()
                                    .find(|(_, status, _)| *status == StrikeStatus::End)
                                    .unwrap();

                                let strike_index = strike_prefix
                                    .borrow()
                                    .iter()
                                    .enumerate()
                                    .find_map(|(index, strike)| {
                                        strike
                                            .is_some_and(|(id, ..)| id == strike_id)
                                            .then_some(index)
                                    })
                                    .unwrap();

                                // INDENT: "<strike-prefix><horizontal><left-to-bottom>"
                                // INDENT: "<strike-prefix>            <top-to-bottom>"
                                let mut wrote = false;
                                indent!(writer, strike_prefix_width + 1 + label_range.end, with: |writer: &mut dyn fmt::Write| {
                                    for strike in strike_prefix.borrow().iter().take(if !wrote { strike_index } else { usize::MAX }) {
                                        match strike {
                                            Some((
                                                _,
                                                StrikeStatus::Start | StrikeStatus::End,
                                                label,
                                            )) => {
                                                write!(
                                                    writer,
                                                    "{symbol}",
                                                    symbol = TOP_TO_BOTTOM.paint(label.style_in(report.severity))
                                                )?;
                                            },

                                            _ => {
                                                write_strike(writer, strike)?;
                                            },
                                        }
                                    }

                                    if !wrote {
                                        write_strike(writer, &Some(strike))?;
                                    }

                                    for _ in 0..if !wrote { strike_prefix_width - strike_index - 1 } else { 0 } + label_range.end
                                    {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = if !wrote { LEFT_TO_RIGHT } else { ' ' }.paint(label_severity.style_in(report.severity))
                                        )?;
                                    }

                                    write!(
                                        writer,
                                        "{symbol}",
                                        symbol = if !wrote { LEFT_TO_BOTTOM } else { TOP_TO_BOTTOM }.paint(label_severity.style_in(report.severity))
                                    )?;

                                    wrote = true;
                                    strike_prefix.borrow_mut()[strike_index] = None;

                                    Ok(strike_prefix_width + 1 + label_range.end)
                                });

                                write_wrapped(
                                    writer,
                                    [label_text
                                        .as_ref()
                                        .paint(label_severity.style_in(report.severity))]
                                    .into_iter(),
                                )?;

                                writeln!(writer)?;
                            },

                            LabelRange::Inline(_range) => todo!(),
                        }
                    }
                }
            }
        }

        // = help: foo bar
        {
            *line_current.borrow_mut() = None;

            // Dedent that separator as we are going to align.
            dedent!(
                writer,
                if line_number_width == 0 {
                    line_number_width + 3
                } else {
                    line_number_width + 1
                }
            );

            for point in &report.points {
                // INDENT: "= "
                indent!(writer, header: "=".paint(STYLE_GUTTER));

                // INDENT: "note: "
                indent!(writer, header: &point.title);

                write_wrapped(writer, [point.text.as_ref().new()].into_iter())?;
            }
        }

        Ok(())
    }
}

fn number_width(number: usize) -> usize {
    if number == 0 {
        1
    } else {
        (number as f64).log10() as usize + 1
    }
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

/// Given a list of ranges which refer to the given content and their associated
/// levels (primary and secondary), resolves the colors for every part, giving
/// the primary color precedence over the secondary color in an overlap.
pub(crate) fn resolve_style<'a>(
    content: &'a str,
    styles: &'a mut [(ops::Range<usize>, LabelSeverity)],
    severity: ReportSeverity,
) -> impl Iterator<Item = yansi::Painted<&'a str>> + 'a {
    styles.sort_by(|(a_range, a_severity), (b_range, b_severity)| {
        match (a_range.start.cmp(&b_range.start), a_severity, b_severity) {
            (cmp::Ordering::Equal, LabelSeverity::Primary, LabelSeverity::Secondary) => {
                cmp::Ordering::Less
            },
            (cmp::Ordering::Equal, LabelSeverity::Secondary, LabelSeverity::Primary) => {
                cmp::Ordering::Greater
            },
            (ordering, ..) => ordering,
        }
    });

    gen move {
        let mut offset: usize = 0;
        let mut style_offset: usize = 0;

        while offset < content.len() {
            let current_style = styles[style_offset..]
                .iter()
                .enumerate()
                .find(|(_, (range, _))| range.start <= offset && offset < range.end);

            match current_style {
                Some((relative_offset, (range, level))) => {
                    style_offset += relative_offset;

                    let next_primary = (*level == LabelSeverity::Secondary)
                        .then(|| {
                            styles[style_offset..]
                                .iter()
                                .enumerate()
                                .take_while(|(_, (r, _))| r.start <= range.end)
                                .find(|(_, (r, label))| {
                                    *label == LabelSeverity::Primary && r.start > offset
                                })
                        })
                        .flatten();

                    match next_primary {
                        Some((relative_offset, (range, ..))) => {
                            style_offset += relative_offset;

                            yield content[offset..range.start].paint(level.style_in(severity));
                            offset = range.start;
                        },

                        None => {
                            yield content[offset..range.end].paint(level.style_in(severity));
                            offset = range.end;
                        },
                    }
                },

                None => {
                    let (relative_offset, next_offset) = styles[style_offset..]
                        .iter()
                        .enumerate()
                        .filter(|(_, (range, _))| range.start > offset)
                        .map(|(relative_offset, (range, ..))| (relative_offset, range.start))
                        .next()
                        .unwrap_or((styles.len() - style_offset, content.len()));

                    style_offset += relative_offset;

                    yield (&content[offset..next_offset]).new();
                    offset = next_offset;
                },
            }
        }
    }
}

pub(crate) fn write_wrapped<'a>(
    writer: &'a mut dyn fmt::Write,
    parts: impl Iterator<Item = yansi::Painted<&'a str>>,
) -> fmt::Result {
    const LINE_WIDTH_MAX: usize = 120;

    let mut line_width: usize = 0;

    use None as Space;
    use Some as Word;

    parts
        .flat_map(|part| {
            part.value
                .split_whitespace()
                .map(move |word| Word(word.paint(part.style)))
                .intersperse(Space)
        })
        .try_for_each(|part| {
            match part {
                Word(word) => {
                    let word_width = word.value.width();

                    if line_width != 0 && line_width + 1 + word_width > LINE_WIDTH_MAX {
                        writeln!(writer)?;
                        line_width = 0;
                    }

                    write!(writer, "{word}")?;
                    line_width += word_width;
                },

                Space => {
                    if line_width != 0 {
                        write!(writer, " ")?;
                        line_width += 1;
                    }
                },
            }

            Ok(())
        })?;

    Ok(())
}

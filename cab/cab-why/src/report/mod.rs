mod label;
mod point;
mod position;

use std::{
    borrow::Cow,
    cell::RefCell,
    cmp,
    fmt::{
        self,
        Write as _,
    },
    iter,
    ops,
};

use smallvec::SmallVec;
use yansi::Paint;

pub use self::{
    label::*,
    point::*,
    position::*,
};
use crate::{
    Size,
    Span,
    dedent,
    indent,
    into,
    wrapln,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportSeverity {
    Note,
    Warn,
    Error,
    Bug,
}

impl ReportSeverity {
    pub fn header(self) -> yansi::Painted<&'static str> {
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

#[derive(Debug, Clone)]
pub struct Report {
    pub severity: ReportSeverity,
    pub title: Cow<'static, str>,
    pub labels: SmallVec<Label, 2>,
    pub points: SmallVec<Point, 2>,
}

impl Report {
    pub fn new(severity: ReportSeverity, title: impl Into<Cow<'static, str>>) -> Self {
        into!(title);

        Self {
            title,
            severity,
            labels: SmallVec::new(),
            points: SmallVec::new(),
        }
    }

    pub fn note(title: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ReportSeverity::Note, title)
    }

    pub fn warn(title: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ReportSeverity::Warn, title)
    }

    pub fn error(title: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ReportSeverity::Error, title)
    }

    pub fn bug(title: impl Into<Cow<'static, str>>) -> Self {
        Self::new(ReportSeverity::Bug, title)
    }

    pub fn is_empty(&self) -> bool {
        self.labels.is_empty() && self.points.is_empty()
    }

    pub fn push_label(&mut self, label: Label) {
        self.labels.push(label)
    }

    pub fn push_primary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
        self.labels.push(Label::primary(span, text));
    }

    pub fn primary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
        self.push_primary(span, text);
        self
    }

    pub fn push_secondary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
        self.labels.push(Label::secondary(span, text));
    }

    pub fn secondary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
        self.push_secondary(span, text);
        self
    }

    pub fn point(mut self, point: Point) -> Self {
        self.points.push(point);
        self
    }

    pub fn push_tip(&mut self, text: impl Into<Cow<'static, str>>) {
        self.points.push(Point::tip(text));
    }

    pub fn tip(self, text: impl Into<Cow<'static, str>>) -> Self {
        self.point(Point::tip(text))
    }

    pub fn push_help(&mut self, text: impl Into<Cow<'static, str>>) {
        self.points.push(Point::help(text));
    }

    pub fn help(self, text: impl Into<Cow<'static, str>>) -> Self {
        self.point(Point::help(text))
    }

    // TODO: Use SyntaxText.
    pub fn with<'a>(&'a self, location: impl fmt::Display + 'a, source: &'a str) -> impl fmt::Display + 'a {
        ReportDisplay {
            report: self,
            location,
            source,
        }
    }
}

struct ReportDisplay<'a, L: fmt::Display> {
    report: &'a Report,
    location: L,
    source: &'a str,
}

fn number_width(number: u32) -> usize {
    if number == 0 {
        1
    } else {
        (number as f64).log10() as usize + 1
    }
}

fn extend_to_line_boundaries(source: &str, mut span: Span) -> Span {
    while *span.start > 0
        && source
            .as_bytes()
            .get(*span.start as usize - 1)
            .is_some_and(|&c| c != b'\n')
    {
        span.start -= 1u32;
    }

    while source.as_bytes().get(*span.end as usize).is_some_and(|&c| c != b'\n') {
        span.end += 1u32;
    }

    span
}

/// Given a list of spans which refer to the given content and their associated
/// levels (primary and secondary), resolves the colors for every part, giving
/// the primary color precedence over the secondary color in an overlap.
pub(crate) fn resolve_style<'a>(
    content: &'a str,
    styles: &'a mut [(Span, LabelSeverity)],
    severity: ReportSeverity,
) -> impl Iterator<Item = yansi::Painted<&'a str>> + 'a {
    styles.sort_by(|(a_span, a_severity), (b_span, b_severity)| {
        match (a_span.start.cmp(&b_span.start), a_severity, b_severity) {
            (cmp::Ordering::Equal, LabelSeverity::Primary, LabelSeverity::Secondary) => cmp::Ordering::Less,
            (cmp::Ordering::Equal, LabelSeverity::Secondary, LabelSeverity::Primary) => cmp::Ordering::Greater,
            (ordering, ..) => ordering,
        }
    });

    gen move {
        let mut offset = Size::new(0u32);
        let mut style_offset: usize = 0;

        while offset < content.len().into() {
            let current_style = styles[style_offset..]
                .iter()
                .enumerate()
                .find(|(_, (span, _))| span.start <= offset && offset < span.end);

            match current_style {
                Some((relative_offset, (span, level))) => {
                    style_offset += relative_offset;

                    let next_primary = (*level == LabelSeverity::Secondary)
                        .then(|| {
                            styles[style_offset..]
                                .iter()
                                .enumerate()
                                .take_while(|(_, (r, _))| r.start <= span.end)
                                .find(|(_, (r, label))| *label == LabelSeverity::Primary && r.start > offset)
                        })
                        .flatten();

                    match next_primary {
                        Some((relative_offset, (span, ..))) => {
                            style_offset += relative_offset;

                            yield content[Span::std(offset, span.start)].paint(level.style_in(severity));
                            offset = span.start;
                        },

                        None => {
                            yield content[Span::std(offset, span.end)].paint(level.style_in(severity));
                            offset = span.end;
                        },
                    }
                },

                None => {
                    let (relative_offset, next_offset) = styles[style_offset..]
                        .iter()
                        .enumerate()
                        .filter(|(_, (span, _))| span.start > offset)
                        .map(|(relative_offset, (span, ..))| (relative_offset, span.start))
                        .next()
                        .unwrap_or((styles.len() - style_offset, content.len().into()));

                    style_offset += relative_offset;

                    yield (&content[Span::std(offset, next_offset)]).new();
                    offset = next_offset;
                },
            }
        }
    }
}
impl<L: fmt::Display> fmt::Display for ReportDisplay<'_, L> {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        const BOTTOM_TO_RIGHT: char = '┏';
        const TOP_TO_BOTTOM: char = '┃';
        const TOP_TO_BOTTOM_PARTIAL: char = '┇';
        const TOP_TO_RIGHT: char = '┗';
        const TOP_LEFT_TO_RIGHT: char = '╲';
        const LEFT_TO_RIGHT: char = '━';
        const LEFT_TO_TOP_BOTTOM: char = '┫';
        const DOT: char = '·';

        const STYLE_GUTTER: yansi::Style = yansi::Style::new().blue();
        const STYLE_HEADER_PATH: yansi::Style = yansi::Style::new().green();
        const STYLE_HEADER_POSITION: yansi::Style = yansi::Style::new().blue();

        let Self {
            report,
            location: file,
            source,
        } = self;

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct StrikeId(u8);

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum StrikeStatus {
            Start,
            Continue,
            End,
        }

        type Strike = (StrikeId, StrikeStatus, LabelSeverity);

        #[derive(Debug, Clone, PartialEq, Eq)]
        enum LabelSpan {
            FromStart(ops::RangeTo<Size>),
            Inline(Span),
        }

        type LabelDynamic<'a> = (LabelSpan, &'a Cow<'static, str>, LabelSeverity);

        #[derive(Debug, Clone)]
        struct Line<'a> {
            number: u32,

            content: &'a str,
            styles: SmallVec<(Span, LabelSeverity), 4>,

            strikes: SmallVec<Strike, 3>,
            labels: SmallVec<LabelDynamic<'a>, 2>,
        }

        let mut labels: SmallVec<_, 2> = report
            .labels
            .clone()
            .into_iter()
            .map(|label| (Position::of(label.span, source), label))
            .collect();

        labels.sort_by(|((a_start, a_end), _), ((b_start, b_end), _)| {
            a_start
                .line
                .cmp(&b_start.line)
                .then_with(|| a_end.column.cmp(&b_end.column))
        });

        let mut lines: SmallVec<Line, 2> = SmallVec::new();

        for (label_index, ((label_start, label_end), label)) in labels.iter().enumerate() {
            let label_is_multiline = label_start.line != label_end.line;

            let label_span_extended = extend_to_line_boundaries(source, label.span);

            for (line_number, line) in (label_start.line.get()..).zip(source[label_span_extended.as_std()].split('\n'))
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
                    let left = label_span_extended.start;

                    let start = label.span.start;
                    let end = label.span.end;

                    let span = Span::new(start - left, end - left);

                    line.labels.push((LabelSpan::Inline(span), &label.text, label.level));

                    line.styles.push((span, label.level));

                    continue;
                }

                let strike_status = match line_number {
                    n if n == label_start.line.get() => StrikeStatus::Start,
                    n if n == label_end.line.get() => StrikeStatus::End,
                    _ => StrikeStatus::Continue,
                };

                line.strikes.push((
                    StrikeId(label_index.try_into().expect("too many overlapping multiline labels")),
                    strike_status,
                    label.level,
                ));

                let line_is_first = line_number == label_start.line.get();
                let line_is_last = line_number == label_end.line.get();

                match (line_is_first, line_is_last) {
                    (true, false) => {
                        let left = label_span_extended.start;

                        let start = label.span.start;
                        let end = source[start.into()..]
                            .find('\n')
                            .map_or(source.len().into(), |index| start + index);

                        let span = Span::new(start - left, end - left);

                        line.styles.push((span, label.level));
                    },

                    (false, false) => {
                        line.styles.push((Span::up_to(line.content.len()), label.level));
                    },

                    (false, true) => {
                        let right = label_span_extended.end;

                        let start = Size::new(0u32);
                        let end = Size::new(line.content.len()) - (right - label.span.end);

                        let span = Span::new(start, end);

                        line.labels
                            .push((LabelSpan::FromStart(..span.end), &label.text, label.level));

                        line.styles.push((span, label.level));
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

        let strike_prefix_width = lines.iter().map(|line| line.strikes.len()).max().unwrap_or(0);

        let strike_prefix = RefCell::new(SmallVec::<_, 3>::from_iter(iter::repeat_n(
            None::<Strike>,
            strike_prefix_width,
        )));

        {
            // INDENT: "note: "
            indent!(writer, header = report.severity.header());

            wrapln(writer, [report.title.as_ref().bold()].into_iter())?;
        }

        // INDENT: "123 | "
        let line_number = RefCell::new(None::<(u32, bool)>);
        let line_number_previous = RefCell::new(None::<u32>);
        indent!(
            writer,
            line_number_width + 3,
            with = |writer: &mut dyn fmt::Write| {
                let Some((line_number, should_write_number)) = *line_number.borrow() else {
                    return Ok(0);
                };
                let mut line_number_previous = line_number_previous.borrow_mut();

                STYLE_GUTTER.fmt_prefix(writer)?;

                if !should_write_number {
                    write!(writer, "{:>line_number_width$}", "")?;
                } else if *line_number_previous == Some(line_number) {
                    // Continuation line.
                    let dot_width = number_width(line_number);

                    write!(
                        writer,
                        "{:>space_width$}",
                        "",
                        space_width = line_number_width - dot_width
                    )?;

                    for _ in 0..dot_width {
                        write!(writer, "{DOT}")?;
                    }
                } else {
                    // New line.

                    if line_number_previous.is_some_and(|number| number + 1 != line_number) {
                        // Non-incremental jump.
                        writeln!(writer, "{:>line_number_width$} {TOP_TO_BOTTOM_PARTIAL} ", "")?;
                    }

                    write!(writer, "{line_number:>line_number_width$}", line_number = line_number)?;
                }

                write!(writer, " {TOP_TO_BOTTOM} ")?;
                STYLE_GUTTER.fmt_suffix(writer)?;

                *line_number_previous = Some(line_number);
                Ok(line_number_width + 3)
            }
        );

        if let Some(((label_start, _), _)) = labels.first() {
            // DEDENT: "| "
            dedent!(writer, 2);

            // INDENT: "┏━━━ ".
            indent!(
                writer,
                header = const_str::concat!(BOTTOM_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT)
                    .paint(STYLE_GUTTER)
            );

            STYLE_HEADER_PATH.fmt_prefix(writer)?;
            write!(writer, "{file}")?;
            STYLE_HEADER_PATH.fmt_suffix(writer)?;

            let line_number = label_start.line.paint(STYLE_HEADER_POSITION);
            let column_number = label_start.column.paint(STYLE_HEADER_POSITION);
            writeln!(writer, ":{line_number}:{column_number}")?;
        }

        {
            // INDENT: "<strikes-prefix> "
            indent!(
                writer,
                strike_prefix_width + 1,
                with = |writer: &mut dyn fmt::Write| {
                    let mut strike_override = None::<yansi::Painted<&char>>;

                    for strike_slot in &*strike_prefix.borrow() {
                        let Some((_, strike_status, strike_severity)) = *strike_slot else {
                            match strike_override {
                                Some(strike) => write!(writer, "{strike}")?,
                                None => write!(writer, " ")?,
                            }
                            continue;
                        };

                        match strike_status {
                            StrikeStatus::Start => {
                                write!(
                                    writer,
                                    "{symbol}",
                                    symbol = BOTTOM_TO_RIGHT.paint(strike_severity.style_in(report.severity))
                                )?;

                                strike_override = Some(LEFT_TO_RIGHT.paint(strike_severity.style_in(report.severity)));
                            },

                            StrikeStatus::Continue | StrikeStatus::End if let Some(strike) = strike_override => {
                                write!(writer, "{strike}")?;
                            },

                            StrikeStatus::Continue | StrikeStatus::End => {
                                write!(
                                    writer,
                                    "{symbol}",
                                    symbol = TOP_TO_BOTTOM.paint(strike_severity.style_in(report.severity))
                                )?;
                            },
                        }
                    }

                    if let Some(strike) = strike_override {
                        write!(writer, "{strike}")?;

                        Ok(strike_prefix_width + 1)
                    } else {
                        Ok(strike_prefix_width)
                    }
                }
            );

            for (line_index, line) in lines.iter_mut().enumerate() {
                if line_index == 0 {
                    *line_number.borrow_mut() = Some((0, false));

                    writer.write_indent()?;
                    writeln!(writer)?;

                    *line_number_previous.borrow_mut() = None;
                }

                {
                    let mut strike_prefix = strike_prefix.borrow_mut();

                    for strike_new @ (strike_id, ..) in &line.strikes {
                        match strike_prefix.iter_mut().flatten().find(|(id, ..)| id == strike_id) {
                            Some(strike) => *strike = *strike_new,

                            None => {
                                *strike_prefix.iter_mut().find(|slot| slot.is_none()).unwrap() = Some(*strike_new);
                            },
                        }
                    }
                }

                {
                    *line_number.borrow_mut() = Some((line.number, true));

                    writer.write_indent()?;
                    wrapln(writer, resolve_style(line.content, &mut line.styles, report.severity))?;

                    *line_number.borrow_mut() = Some((line.number, false));
                }

                for (label_span, label_text, label_severity) in line.labels.iter().rev() {
                    match label_span {
                        LabelSpan::FromStart(label_span) => {
                            let label_span_end = (*label_span.end).min(60) as usize;

                            let &(strike_id, _, strike_severity) = strike_prefix
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
                                    strike.is_some_and(|(id, ..)| id == strike_id).then_some(index)
                                })
                                .unwrap();

                            // DEDENT: "<strike-prefix> "
                            dedent!(writer);

                            // INDENT: "<strike-prefix><horizontal><left-to-bottom> "
                            // INDENT: "<strike-prefix>            <top--to-bottom> "
                            let mut wrote = false;
                            indent!(
                                writer,
                                strike_prefix_width + 1 + label_span_end + 1,
                                with = |writer: &mut dyn fmt::Write| {
                                    for strike in strike_prefix.borrow().iter().take(if !wrote {
                                        strike_index
                                    } else {
                                        usize::MAX
                                    }) {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = match strike {
                                                Some((.., severity)) =>
                                                    TOP_TO_BOTTOM.paint(severity.style_in(report.severity)),
                                                None => (&' ').new(),
                                            }
                                        )?;
                                    }

                                    if !wrote {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = TOP_TO_RIGHT.paint(strike_severity.style_in(report.severity))
                                        )?;
                                    }

                                    for _ in 0..if !wrote {
                                        strike_prefix_width - strike_index - 1
                                    } else {
                                        0
                                    } + label_span_end
                                    {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = if !wrote { LEFT_TO_RIGHT } else { ' ' }
                                                .paint(label_severity.style_in(report.severity))
                                        )?;
                                    }

                                    write!(
                                        writer,
                                        "{symbol}",
                                        symbol = if !wrote { LEFT_TO_TOP_BOTTOM } else { TOP_TO_BOTTOM }
                                            .paint(label_severity.style_in(report.severity))
                                    )?;

                                    wrote = true;
                                    strike_prefix.borrow_mut()[strike_index] = None;

                                    Ok(strike_prefix_width + 1 + label_span_end)
                                }
                            );

                            wrapln(
                                writer,
                                [label_text.as_ref().paint(label_severity.style_in(report.severity))].into_iter(),
                            )?;
                        },

                        LabelSpan::Inline(label_span) => {
                            let label_span_end = (*label_span.end).min(60) as usize;

                            // DEDENT: "<strike-prefix> "
                            dedent!(writer);

                            // INDENT: "<strike-prefix>"
                            indent!(
                                writer,
                                strike_prefix_width + 1,
                                with = |writer: &mut dyn fmt::Write| {
                                    for strike in &*strike_prefix.borrow() {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = match strike {
                                                Some((.., severity)) =>
                                                    TOP_TO_BOTTOM.paint(severity.style_in(report.severity)),
                                                None => (&' ').new(),
                                            }
                                        )?;
                                    }

                                    Ok(strike_prefix_width)
                                }
                            );

                            // INDENT: "<prefix-spaces>"
                            indent!(writer, *label_span.start as u16);

                            // INDENT: "<horizontal><left-to-bottom> "
                            // INDENT: "            <top--to-bottom> "
                            let underline_width = label_span_end - *label_span.start as usize;
                            let mut wrote = false;
                            indent!(
                                writer,
                                underline_width.max(1) + 1,
                                with = |writer: &mut dyn fmt::Write| {
                                    for index in 0..underline_width.saturating_sub(1) {
                                        write!(
                                            writer,
                                            "{symbol}",
                                            symbol = if wrote {
                                                ' '
                                            } else if index == 0 {
                                                TOP_TO_RIGHT
                                            } else {
                                                LEFT_TO_RIGHT
                                            }
                                            .paint(label_severity.style_in(report.severity)),
                                        )?;
                                    }

                                    write!(
                                        writer,
                                        "{symbol}",
                                        symbol = match underline_width {
                                            _ if wrote => TOP_TO_BOTTOM,
                                            0 => TOP_LEFT_TO_RIGHT,
                                            1 => TOP_TO_BOTTOM,
                                            _ => LEFT_TO_TOP_BOTTOM,
                                        }
                                        .paint(label_severity.style_in(report.severity)),
                                    )?;

                                    wrote = true;
                                    Ok(underline_width.max(1))
                                }
                            );

                            wrapln(
                                writer,
                                [label_text.as_ref().paint(label_severity.style_in(report.severity))].into_iter(),
                            )?;
                        },
                    }
                }
            }
        }

        // = help: foo bar
        {
            if !report.points.is_empty() {
                writer.write_indent()?;
                writeln!(writer)?;
            }

            // DEDENT: "| "
            dedent!(writer, 2);

            for point in &report.points {
                // INDENT: "= "
                indent!(writer, header = "=".paint(STYLE_GUTTER));

                // INDENT: "note: "
                indent!(writer, header = &point.title);

                wrapln(writer, [point.text.as_ref().new()].into_iter())?;
            }
        }

        Ok(())
    }
}

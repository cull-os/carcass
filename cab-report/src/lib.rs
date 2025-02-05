//! Diagnostic reporting utilities.
#![feature(gen_blocks, iter_intersperse, try_blocks)]

mod indent;
mod position;

use std::{
    borrow,
    cmp,
    fmt::{
        self,
        Write as _,
    },
    io::Write as _,
    ops,
};

use indent::*;
use position::*;
use unicode_width::UnicodeWidthStr as _;
use yansi::Paint;

type CowStr<'a> = borrow::Cow<'a, str>;

pub fn init(level_filter: log::LevelFilter) {
    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    env_logger::Builder::new()
        .filter_level(level_filter)
        .format(|buffer, record| {
            let level = match record.level() {
                log::Level::Error => "error:".red().bold(),
                log::Level::Warn => "warn:".yellow().bold(),
                log::Level::Info => "info:".green().bold(),
                log::Level::Debug => "debug:".blue().bold(),
                log::Level::Trace => "trace:".cyan().bold(),
            };

            writeln!(buffer, "{level} {arguments}", arguments = record.args())
        })
        .target(env_logger::Target::Stdout)
        .init();
}

fn write_wrapped(writer: &mut dyn fmt::Write, s: yansi::Painted<&str>) -> fmt::Result {
    const LINE_WIDTH_MAX: usize = 80;

    let mut line_width = 0;

    s.style.fmt_prefix(writer)?;

    for word in s.value.split_whitespace() {
        let word_width = word.width();

        if line_width != 0 && line_width + 1 + word_width > LINE_WIDTH_MAX {
            writeln!(writer)?;
            line_width = 0;
        }

        if line_width != 0 {
            write!(writer, " ")?;
            line_width += 1;
        }

        write!(writer, "{word}")?;
        line_width += word_width;
    }

    s.style.fmt_suffix(writer)?;

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct RangeEvent {
    level: LabelLevel,
    position: Position,
    is_start: bool,
}

impl RangeEvent {
    fn start(level: LabelLevel, position: Position) -> Self {
        Self {
            level,
            position,
            is_start: true,
        }
    }

    fn end(level: LabelLevel, position: Position) -> Self {
        Self {
            level,
            position,
            is_start: false,
        }
    }
}

impl PartialOrd for RangeEvent {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RangeEvent {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        if self.is_start == other.is_start {
            self.position.line.cmp(&other.position.line)
        } else if self.is_start {
            cmp::Ordering::Less
        } else {
            cmp::Ordering::Greater
        }
    }
}

fn max_offset_and_events(
    ranges: &[(ops::RangeInclusive<usize>, LabelLevel)],
) -> (usize, Vec<RangeEvent>) {
    let mut events = Vec::with_capacity(ranges.len());

    for (discrim, (range, level)) in ranges.iter().enumerate() {
        events.push(RangeEvent::start(*level, Position {
            column: discrim + 1,
            line: *range.start(),
        }));

        events.push(RangeEvent::end(*level, Position {
            column: discrim + 1,
            line: *range.end(),
        }));
    }

    events.sort();

    let mut max_offset: usize = 0;
    let mut offset = 0;

    let mut starts = [0].repeat(ranges.len());

    for event in &mut events {
        let discrim = &mut event.position.column;

        if event.is_start {
            starts[offset] = *discrim;

            offset += 1;
            *discrim = offset;
        } else {
            offset -= 1;
            *discrim = starts[*discrim - 1];
        }

        max_offset = max_offset.max(offset);
    }

    (max_offset, events)
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

#[derive(Debug)]
pub struct File<'a> {
    // TODO: Use a real island type.
    pub island: CowStr<'a>,
    pub path: CowStr<'a>,
    pub source: CowStr<'a>,
}

pub struct Message<'a> {
    title: CowStr<'a>,
    level: log::Level,
    labels: Vec<Label<'a>>,
    tips: Vec<Tip<'a>>,
}

impl<'a> Message<'a> {
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

    pub fn push_primary(&mut self, range: ops::Range<usize>, message: impl Into<CowStr<'a>>) {
        self.push_label(Label::primary(range, message));
    }

    pub fn primary(mut self, range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        self.push_primary(range, message);
        self
    }

    pub fn push_secondary(&mut self, range: ops::Range<usize>, message: impl Into<CowStr<'a>>) {
        self.push_label(Label::secondary(range, message));
    }

    pub fn secondary(mut self, range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        self.push_secondary(range, message);
        self
    }

    pub fn push_tip(&mut self, tip: Tip<'a>) {
        self.tips.push(tip)
    }

    pub fn tip(mut self, tip: Tip<'a>) -> Self {
        self.push_tip(tip);
        self
    }

    pub fn push_note(&mut self, message: impl Into<CowStr<'a>>) {
        self.push_tip(Tip::note(message));
    }

    pub fn note(self, message: impl Into<CowStr<'a>>) -> Self {
        self.tip(Tip::note(message))
    }

    pub fn push_help(&mut self, message: impl Into<CowStr<'a>>) {
        self.push_tip(Tip::help(message));
    }

    pub fn help(self, message: impl Into<CowStr<'a>>) -> Self {
        self.tip(Tip::help(message))
    }

    pub fn log_with(&'a self, file: &'a File<'a>) -> impl fmt::Display {
        struct WithFile<'a>(&'a Message<'a>, &'a File<'a>);

        impl fmt::Display for WithFile<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let Message {
                    title,
                    labels,
                    tips,
                    ..
                } = self.0;

                let file = self.1;

                writeln!(formatter, "{title}", title = title.bright_white().bold())?;

                let mut labels: Vec<_> = labels
                    .clone()
                    .into_iter()
                    .map(|label| {
                        let positions = Position::of(&label.range, &file.source);
                        (label, positions)
                    })
                    .collect();

                labels.sort_by_key(|(label, _)| label.range.start);

                let line_number_width: usize = labels
                    .iter()
                    .max_by_key(|(_, (_, end))| end.line)
                    .map(|(_, (_, end))| {
                        if end.line == 0 {
                            1
                        } else {
                            (end.line as f64).log10() as usize + 1
                        }
                    })
                    .unwrap_or(0);

                // +1 for the space between the number and +
                indent!(formatter, line_number_width + 1);

                if let Some((_, (start, _))) = labels.first() {
                    indent!(formatter, header: "+-->".blue());

                    {
                        let File { island, path, .. } = file;
                        let style = yansi::Color::Green.foreground();

                        style.fmt_prefix(formatter)?;
                        write!(formatter, "{island}{path}")?;
                        style.fmt_suffix(formatter)?;
                    }

                    {
                        let Position { line, column } = start;

                        write!(
                            formatter,
                            ":{line}:{column}",
                            line = line.blue(),
                            column = column.blue()
                        )?;
                    }
                }

                {
                    dedent!(formatter);
                    writeln!(formatter)?;

                    let source = &*file.source;

                    let mut lines: Vec<(usize, &str)> = Vec::new();

                    let mut verticals: Vec<(ops::RangeInclusive<usize>, LabelLevel)> = Vec::new();
                    let mut horizontals: Vec<(usize, &Label<'_>)> = Vec::new();

                    for (label, (start, end)) in &labels {
                        if start.line == end.line {
                            horizontals.push((start.line, label));
                        } else {
                            verticals.push((start.line..=end.line, label.level));
                            horizontals.push((end.line, label));
                        }

                        let line_range = extend_to_line_boundaries(source, label.range.clone());
                        let line_slice = &source[line_range];

                        for (line_number, line) in (start.line..).zip(line_slice.split('\n')) {
                            if lines.iter().any(|(number, _)| *number == line_number) {
                                continue;
                            }

                            lines.push((line_number, line));
                        }
                    }

                    let (prefix_indent_width, mut events) = max_offset_and_events(&verticals);

                    events.reverse();

                    let mut prefix_indent = [(&' ').new()].repeat(prefix_indent_width);
                    let mut prefix_indent_patch = prefix_indent.clone();

                    let mut lines = lines.into_iter().peekable();
                    while let Some((line_number, line)) = lines.next() {
                        write!(
                            formatter,
                            "{line_number:>line_number_width$} {separator} ",
                            line_number = line_number.blue().bold(),
                            separator = "|".blue(),
                        )?;

                        for (patch, real) in
                            prefix_indent_patch.iter_mut().zip(prefix_indent.iter_mut())
                        {
                            *real = *patch;
                            *patch = (&' ').new();
                        }

                        while events
                            .last()
                            .is_some_and(|event| event.position.line == line_number)
                        {
                            let event = events.pop().unwrap();
                            let index = event.position.column - 1;
                            let style = event.level.style();

                            (prefix_indent[index], prefix_indent_patch[index]) = if event.is_start {
                                ('/'.paint(style), '|'.paint(style))
                            } else {
                                ('\\'.paint(style), (&' ').new())
                            }
                        }

                        // Reverse for right-alignment.
                        for c in prefix_indent.iter().rev() {
                            write!(formatter, "{c}")?;
                        }

                        write!(formatter, "{line}")?;

                        if lines.peek().is_some() {
                            writeln!(formatter)?;
                        }
                        // TODO: print labels
                    }
                }

                writeln!(formatter)?;

                for tip in tips {
                    indent!(formatter, header: "=".blue());
                    writeln!(formatter, "{tip}")?;
                }

                Ok(())
            }
        }

        log::log!(self.level, "{with_file}", with_file = WithFile(self, file));

        WithFile(self, file)
    }
}

#[derive(Debug, Clone)]
pub struct Label<'a> {
    range: ops::Range<usize>,
    level: LabelLevel,
    message: CowStr<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelLevel {
    Primary,
    Secondary,
}

impl LabelLevel {
    pub fn style(&self) -> yansi::Style {
        match self {
            LabelLevel::Primary => yansi::Color::Red,
            LabelLevel::Secondary => yansi::Color::Green,
        }
        .foreground()
    }
}

impl<'a> Label<'a> {
    pub fn new(
        range: ops::Range<usize>,
        message: impl Into<CowStr<'a>>,
        level: LabelLevel,
    ) -> Self {
        Self {
            range,
            message: message.into(),
            level,
        }
    }

    pub fn primary(range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, message, LabelLevel::Primary)
    }

    pub fn secondary(range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, message, LabelLevel::Secondary)
    }
}

impl fmt::Display for Label<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_wrapped(formatter, self.message.as_ref().paint(self.level.style()))
    }
}

#[derive(Debug, Clone)]
pub struct Tip<'a> {
    title: yansi::Painted<CowStr<'a>>,
    message: CowStr<'a>,
}

impl<'a> Tip<'a> {
    pub fn new(title: yansi::Painted<CowStr<'a>>, message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title,
            message: message.into(),
        }
    }

    pub fn note(message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("note:").new().magenta().bold(),
            message: message.into(),
        }
    }

    pub fn help(message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("help:").new().cyan().bold(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Tip<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { title, message } = &self;

        indent!(formatter, header: title);

        write_wrapped(formatter, message.as_ref().new())
    }
}

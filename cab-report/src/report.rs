use std::{
    fmt::{
        self,
        Write as _,
    },
    iter,
    ops,
};

use yansi::Paint as _;

use crate::*;

#[derive(Debug)]
struct RangeEvent<'a> {
    label: &'a Label<'a>,
    position: Position,
    is_start: bool,
}

impl<'a> RangeEvent<'a> {
    fn start(label: &'a Label, position: Position) -> Self {
        Self {
            label,
            position,
            is_start: true,
        }
    }

    fn end(label: &'a Label, position: Position) -> Self {
        Self {
            label,
            position,
            is_start: false,
        }
    }
}

impl PartialEq for RangeEvent<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.position == other.position && self.is_start == other.is_start
    }
}

impl Eq for RangeEvent<'_> {}

impl PartialOrd for RangeEvent<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some((self.position.clone(), self.is_start).cmp(&(other.position.clone(), other.is_start)))
    }
}

impl Ord for RangeEvent<'_> {
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

fn max_offset_and_events<'a>(
    ranges: &'a [(ops::RangeInclusive<usize>, &'a Label)],
) -> (usize, Vec<RangeEvent<'a>>) {
    let mut events = Vec::with_capacity(ranges.len());

    for (discrim, (range, level)) in ranges.iter().enumerate() {
        events.push(RangeEvent::start(level, Position {
            column: discrim + 1,
            line: *range.start(),
        }));

        events.push(RangeEvent::end(level, Position {
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

    pub fn log_with(&'a self, file: &'a File<'a>) -> impl fmt::Display {
        struct WithFile<'a>(&'a Report<'a>, &'a File<'a>);

        impl fmt::Display for WithFile<'_> {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                let Report {
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
                    .map(|label| (Position::from(&label.range, file), label))
                    .collect();

                labels.sort_by_key(|(_, label)| label.range.start);

                let line_number_width: usize = labels
                    .iter()
                    .max_by_key(|((_, end), _)| end.line)
                    .map(|((_, end), _)| {
                        if end.line == 0 {
                            1
                        } else {
                            (end.line as f64).log10() as usize + 1
                        }
                    })
                    .unwrap_or(0);

                // +1 for the space between the number and +
                indent!(formatter, line_number_width + 1);

                if let Some(((start, _), _)) = labels.first() {
                    indent!(formatter, header: "┣━━━".blue());

                    {
                        let File { island, path, .. } = file;
                        let style = yansi::Color::Green.foreground();

                        style.fmt_prefix(formatter)?;
                        write!(formatter, "{island}{path}")?;
                        style.fmt_suffix(formatter)?;
                    }

                    {
                        let Position { line, column } = start;

                        writeln!(
                            formatter,
                            ":{line}:{column}",
                            line = line.blue(),
                            column = column.blue()
                        )?;
                    }
                }

                {
                    dedent!(formatter);

                    let source = &*file.source;

                    let mut lines: Vec<(usize, &str)> = Vec::new();

                    let mut verticals: Vec<(ops::RangeInclusive<usize>, &Label)> = Vec::new();
                    let mut horizontals: Vec<(usize, &Label<'_>)> = Vec::new();

                    for ((start, end), label) in &labels {
                        if start.line == end.line {
                            horizontals.push((start.line, label));
                        } else {
                            verticals.push((start.line..=end.line, label));
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

                    let mut line_labels: Vec<_> =
                        iter::repeat(None).take(prefix_indent_width).collect();

                    for (line_number, line) in lines {
                        macro_rules! write_indent {
                            ($writer:ident,number: $write_number:literal) => {
                                indent!($writer, line_number_width + 3, with: move |writer: &mut dyn fmt::Write| {
                                    if $write_number {
                                        write!(writer, "{line_number:>line_number_width$}", line_number = line_number.blue().bold())?;
                                    } else {
                                        write!(writer, "{:>line_number_width$}", "")?;
                                    }

                                    write!(writer, " {separator} ", separator = '┃'.blue())?;

                                    Ok(line_number_width + 3)
                                });
                            };
                        }

                        for (patch, real) in
                            prefix_indent_patch.iter_mut().zip(prefix_indent.iter_mut())
                        {
                            *real = *patch;
                        }

                        while events
                            .last()
                            .is_some_and(|event| event.position.line == line_number)
                        {
                            let event = events.pop().unwrap();
                            let index = event.position.column - 1;
                            let style = event.label.level.style();

                            (prefix_indent[index], prefix_indent_patch[index]) = if event.is_start {
                                for indent in &mut prefix_indent[..index] {
                                    if *indent.value != '┣' {
                                        *indent = '━'.paint(style);
                                    }
                                }

                                ('┣'.paint(style), '┃'.paint(style))
                            } else {
                                ('┃'.paint(style), (&' ').new())
                            };

                            if !event.is_start {
                                line_labels[index] = Some(event.label);
                            }
                        }

                        {
                            let mut wrote = false;

                            indent!(formatter, line_number_width + 3 + prefix_indent_width, with: |writer: &mut dyn fmt::Write| {
                                if wrote {
                                    let dot_width = (line_number as f64).log10() as usize + 1;
                                    write!(writer, "{:>space_width$}", "", space_width = line_number_width - dot_width)?;

                                    for _ in 0..dot_width {
                                        write!(writer, "{dot}", dot = '·'.blue())?;
                                    }
                                } else {
                                    write!(writer, "{line_number:>line_number_width$}", line_number = line_number.blue().bold())?;
                                    wrote = true;
                                }

                                write!(writer, " {separator} ", separator = '┃'.blue())?;

                                // Reverse for right-alignment.
                                for (index, c) in prefix_indent.iter().enumerate().rev() {
                                    write!(writer, "{c}")?;
                                }

                                Ok(line_number_width + 3 + prefix_indent_width)
                            });

                            write_wrapped(formatter, line.new())?;
                            writeln!(formatter)?;
                        }

                        for (index, line_label) in line_labels.iter_mut().enumerate().rev() {
                            let Some(label) = line_label else { continue };

                            write_indent!(formatter, number: false);

                            indent!(formatter, prefix_indent_width, with: |writer: &mut dyn fmt::Write| {
                                for c in prefix_indent.iter().rev() {
                                    write!(writer, "{c}")?;
                                }

                                Ok(prefix_indent_width)
                            });

                            writeln!(formatter, "{label}")?;

                            prefix_indent[index] = (&' ').new();
                            *line_label = None;
                        }
                    }
                }

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

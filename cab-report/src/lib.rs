//! Diagnostic reporting utilities.
#![feature(if_let_guard, iter_intersperse, let_chains, gen_blocks)]

mod file;
mod indent;
mod label;
mod report;
mod tip;

use std::{
    borrow,
    cmp,
    fmt,
    ops,
};

use unicode_width::UnicodeWidthStr as _;
use yansi::Paint as _;

pub use crate::{
    file::*,
    indent::*,
    label::*,
    report::*,
    tip::*,
};

pub(crate) type CowStr<'a> = borrow::Cow<'a, str>;

pub fn init(level_filter: log::LevelFilter) {
    yansi::whenever(yansi::Condition::TTY_AND_COLOR);

    env_logger::Builder::new()
        .filter_level(level_filter)
        .format(|buffer, record| {
            use std::io::Write as _;

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

/// Given a list of ranges which refer to the given content and their associated
/// levels (primary and secondary), resolves the colors for every part, giving
/// the primary color precedence over the secondary color in an overlap.
pub(crate) fn resolve_style<'a>(
    content: &'a str,
    styles: &'a mut [(ops::Range<usize>, LabelLevel)],
) -> impl Iterator<Item = yansi::Painted<&'a str>> + 'a {
    styles.sort_by(|(a_range, a_level), (b_range, b_level)| {
        match (a_range.start.cmp(&b_range.start), a_level, b_level) {
            (cmp::Ordering::Equal, LabelLevel::Primary, LabelLevel::Secondary) => {
                cmp::Ordering::Less
            },
            (cmp::Ordering::Equal, LabelLevel::Secondary, LabelLevel::Primary) => {
                cmp::Ordering::Greater
            },
            (ordering, ..) => ordering,
        }
    });

    gen {
        let mut offset: usize = 0;
        let mut style_offset: usize = 0;

        while offset < content.len() {
            let current_style = styles[style_offset..]
                .iter()
                .enumerate()
                .find(|(.., (range, ..))| range.start <= offset && offset < range.end);

            match current_style {
                Some((relative_offset, (range, level))) => {
                    style_offset += relative_offset;

                    let next_primary = (*level == LabelLevel::Secondary)
                        .then(|| {
                            styles[style_offset..]
                                .iter()
                                .enumerate()
                                .take_while(|(.., (r, ..))| r.start <= range.end)
                                .find(|(.., (r, label))| {
                                    *label == LabelLevel::Primary && r.start > offset
                                })
                        })
                        .flatten();

                    match next_primary {
                        Some((relative_offset, (range, ..))) => {
                            style_offset += relative_offset;

                            yield content[offset..range.start].paint(level.style());
                            offset = range.start;
                        },

                        None => {
                            yield content[offset..range.end].paint(level.style());
                            offset = range.end;
                        },
                    }
                },

                None => {
                    let (relative_offset, next_offset) = styles[style_offset..]
                        .iter()
                        .enumerate()
                        .filter(|(.., (range, ..))| range.start > offset)
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

//! Diagnostic reporting utilities.
#![feature(gen_blocks, iter_intersperse, try_blocks)]

mod file;
mod indent;
mod label;
mod message;
mod position;
mod tip;

use std::{
    borrow,
    cmp,
    fmt,
    io::Write as _,
};

use unicode_width::UnicodeWidthStr as _;
use yansi::Paint as _;

pub use crate::{
    file::*,
    indent::*,
    label::*,
    message::*,
    position::*,
    tip::*,
};

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

pub(crate) type CowStr<'a> = borrow::Cow<'a, str>;

pub(crate) fn write_wrapped(writer: &mut dyn fmt::Write, s: yansi::Painted<&str>) -> fmt::Result {
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

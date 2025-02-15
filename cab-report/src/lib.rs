//! Diagnostic reporting utilities.
#![feature(if_let_guard, iter_intersperse, let_chains, gen_blocks)]

mod file;
mod label;
mod point;
mod report;

use std::cmp;

use yansi::Paint as _;

pub use crate::{
    file::*,
    label::*,
    point::*,
    report::*,
};

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

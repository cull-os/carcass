//! Diagnostic reporting utilities.
#![feature(gen_blocks, if_let_guard)]

mod file;
mod label;
mod point;
mod report;

use std::cmp;

pub use crate::{
    file::*,
    label::*,
    point::*,
    report::*,
};

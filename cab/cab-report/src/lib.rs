//! Diagnostic reporting utilities.
#![feature(gen_blocks, if_let_guard)]

mod label;
mod point;
mod position;
mod report;

use std::cmp;

pub use crate::{
    label::*,
    point::*,
    position::*,
    report::*,
};

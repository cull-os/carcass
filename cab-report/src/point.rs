use cab_text::{
    into,
    paint,
};
use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Point {
    pub title: yansi::Painted<CowStr>,
    pub text: CowStr,
}

impl Point {
    pub fn new(title: yansi::Painted<impl Into<CowStr>>, text: impl Into<CowStr>) -> Self {
        let title = paint(title.value.into(), title.style);
        into!(text);

        Self { title, text }
    }

    pub fn tip(text: impl Into<CowStr>) -> Self {
        Self::new("tip:".magenta().bold(), text)
    }

    pub fn help(text: impl Into<CowStr>) -> Self {
        Self::new("help:".cyan().bold(), text)
    }
}

use cab_text::{
    into,
    paint,
};
use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Point<'a> {
    pub title: yansi::Painted<CowStr<'a>>,
    pub text: CowStr<'a>,
}

impl<'a> Point<'a> {
    pub fn new(title: yansi::Painted<impl Into<CowStr<'a>>>, text: impl Into<CowStr<'a>>) -> Self {
        let title = paint(title.value.into(), title.style);
        into!(text);

        Self { title, text }
    }

    pub fn tip(text: impl Into<CowStr<'a>>) -> Self {
        Self::new("tip:".magenta().bold(), text)
    }

    pub fn help(text: impl Into<CowStr<'a>>) -> Self {
        Self::new("help:".cyan().bold(), text)
    }
}

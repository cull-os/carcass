use std::borrow::Cow;

use cab_text::{
    into,
    paint,
};
use yansi::Paint as _;

#[derive(Debug, Clone)]
pub struct Point {
    pub title: yansi::Painted<Cow<'static, str>>,
    pub text: Cow<'static, str>,
}

impl Point {
    pub fn new(title: yansi::Painted<impl Into<Cow<'static, str>>>, text: impl Into<Cow<'static, str>>) -> Self {
        let title = paint(title.value.into(), title.style);
        into!(text);

        Self { title, text }
    }

    pub fn tip(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new("tip:".magenta().bold(), text)
    }

    pub fn help(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new("help:".cyan().bold(), text)
    }
}

use std::borrow::Cow;

use yansi::Paint as _;

use crate::into;

#[derive(Debug, Clone)]
pub struct Point {
    pub title: yansi::Painted<Cow<'static, str>>,
    pub text: Cow<'static, str>,
}

impl Point {
    pub fn new(title: yansi::Painted<impl Into<Cow<'static, str>>>, text: impl Into<Cow<'static, str>>) -> Self {
        let mut title2 = title.value.into().new();
        title2.style = title.style;

        into!(text);

        Self { title: title2, text }
    }

    pub fn tip(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new("tip:".magenta().bold(), text)
    }

    pub fn help(text: impl Into<Cow<'static, str>>) -> Self {
        Self::new("help:".cyan().bold(), text)
    }
}

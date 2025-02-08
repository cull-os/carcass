use std::{
    borrow,
    fmt,
};

use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Tip<'a> {
    title: yansi::Painted<CowStr<'a>>,
    text: CowStr<'a>,
}

impl<'a> Tip<'a> {
    pub fn new(title: yansi::Painted<CowStr<'a>>, text: impl Into<CowStr<'a>>) -> Self {
        Self {
            title,
            text: text.into(),
        }
    }

    pub fn note(text: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("note:").new().magenta().bold(),
            text: text.into(),
        }
    }

    pub fn help(text: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("help:").new().cyan().bold(),
            text: text.into(),
        }
    }
}

impl fmt::Display for Tip<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { title, text } = &self;

        indent!(formatter, header: title);

        write_wrapped(formatter, text.as_ref().new())
    }
}

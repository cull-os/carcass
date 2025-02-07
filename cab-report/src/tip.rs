use std::{
    borrow,
    fmt,
};

use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Tip<'a> {
    title: yansi::Painted<CowStr<'a>>,
    message: CowStr<'a>,
}

impl<'a> Tip<'a> {
    pub fn new(title: yansi::Painted<CowStr<'a>>, message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title,
            message: message.into(),
        }
    }

    pub fn note(message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("note:").new().magenta().bold(),
            message: message.into(),
        }
    }

    pub fn help(message: impl Into<CowStr<'a>>) -> Self {
        Self {
            title: borrow::Cow::Borrowed("help:").new().cyan().bold(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Tip<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { title, message } = &self;

        indent!(formatter, header: title);

        write_wrapped(formatter, message.as_ref().new())
    }
}

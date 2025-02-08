use std::borrow;

use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Tip<'a> {
    pub(crate) title: yansi::Painted<CowStr<'a>>,
    pub(crate) text: CowStr<'a>,
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

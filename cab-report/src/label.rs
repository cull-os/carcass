use std::{
    fmt,
    ops,
};

use yansi::Paint as _;

use crate::*;

#[derive(Debug, Clone)]
pub struct Label<'a> {
    pub(crate) range: ops::Range<usize>,
    pub(crate) level: LabelLevel,
    message: CowStr<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelLevel {
    Primary,
    Secondary,
}

impl LabelLevel {
    pub fn style(&self) -> yansi::Style {
        match self {
            LabelLevel::Primary => yansi::Color::Red,
            LabelLevel::Secondary => yansi::Color::Green,
        }
        .foreground()
    }
}

impl<'a> Label<'a> {
    pub fn new(
        range: ops::Range<usize>,
        message: impl Into<CowStr<'a>>,
        level: LabelLevel,
    ) -> Self {
        Self {
            range,
            message: message.into(),
            level,
        }
    }

    pub fn primary(range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, message, LabelLevel::Primary)
    }

    pub fn secondary(range: ops::Range<usize>, message: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, message, LabelLevel::Secondary)
    }
}

impl fmt::Display for Label<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_wrapped(formatter, self.message.as_ref().paint(self.level.style()))
    }
}

use std::ops;

use crate::*;

#[derive(Debug, Clone)]
pub struct Label<'a> {
    pub(crate) range: ops::Range<usize>,
    pub(crate) level: LabelLevel,
    pub(crate) text: CowStr<'a>,
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
            LabelLevel::Secondary => yansi::Color::Cyan,
        }
        .foreground()
    }
}

impl<'a> Label<'a> {
    pub fn new(range: ops::Range<usize>, text: impl Into<CowStr<'a>>, level: LabelLevel) -> Self {
        Self {
            range,
            text: text.into(),
            level,
        }
    }

    pub fn primary(range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, text, LabelLevel::Primary)
    }

    pub fn secondary(range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, text, LabelLevel::Secondary)
    }

    pub fn range(&self) -> ops::Range<usize> {
        self.range.clone()
    }
}

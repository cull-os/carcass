use std::ops;

use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSeverity {
    Secondary,
    Primary,
}

impl LabelSeverity {
    pub fn style_in(self, severity: ReportSeverity) -> yansi::Style {
        use yansi::Color::*;

        match (severity, self) {
            (ReportSeverity::Note, LabelSeverity::Secondary) => Blue,
            (ReportSeverity::Note, LabelSeverity::Primary) => Magenta,

            (ReportSeverity::Warn, LabelSeverity::Secondary) => Blue,
            (ReportSeverity::Warn, LabelSeverity::Primary) => Yellow,

            (ReportSeverity::Error, LabelSeverity::Secondary) => Yellow,
            (ReportSeverity::Error, LabelSeverity::Primary) => Red,

            (ReportSeverity::Bug, LabelSeverity::Primary) => Yellow,
            (ReportSeverity::Bug, LabelSeverity::Secondary) => Red,
        }
        .foreground()
    }
}

#[derive(Debug, Clone)]
pub struct Label<'a> {
    pub(crate) range: ops::Range<usize>,
    pub(crate) level: LabelSeverity,
    pub(crate) text: CowStr<'a>,
}

impl<'a> Label<'a> {
    pub fn new(range: ops::Range<usize>, text: impl Into<CowStr<'a>>, level: LabelSeverity) -> Self {
        Self {
            range,
            text: text.into(),
            level,
        }
    }

    pub fn primary(range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, text, LabelSeverity::Primary)
    }

    pub fn secondary(range: ops::Range<usize>, text: impl Into<CowStr<'a>>) -> Self {
        Self::new(range, text, LabelSeverity::Secondary)
    }

    pub fn range(&self) -> ops::Range<usize> {
        self.range.clone()
    }
}

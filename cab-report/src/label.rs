use cab_text::{
    Range,
    into,
};

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

            (ReportSeverity::Bug, LabelSeverity::Secondary) => Yellow,
            (ReportSeverity::Bug, LabelSeverity::Primary) => Red,
        }
        .foreground()
    }
}

#[derive(Debug, Clone)]
pub struct Label {
    pub range: Range,
    pub level: LabelSeverity,
    pub text: CowStr,
}

impl Label {
    #[inline]
    pub fn new(range: impl Into<Range>, text: impl Into<CowStr>, level: LabelSeverity) -> Self {
        into!(range, text);

        Self { range, text, level }
    }

    #[inline]
    pub fn primary(range: impl Into<Range>, text: impl Into<CowStr>) -> Self {
        Self::new(range, text, LabelSeverity::Primary)
    }

    #[inline]
    pub fn secondary(range: impl Into<Range>, text: impl Into<CowStr>) -> Self {
        Self::new(range, text, LabelSeverity::Secondary)
    }
}

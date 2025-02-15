use cab_text::{
    Span,
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
    pub span: Span,
    pub level: LabelSeverity,
    pub text: CowStr,
}

impl Label {
    #[inline]
    pub fn new(span: impl Into<Span>, text: impl Into<CowStr>, level: LabelSeverity) -> Self {
        into!(span, text);

        Self { span, text, level }
    }

    #[inline]
    pub fn primary(span: impl Into<Span>, text: impl Into<CowStr>) -> Self {
        Self::new(span, text, LabelSeverity::Primary)
    }

    #[inline]
    pub fn secondary(span: impl Into<Span>, text: impl Into<CowStr>) -> Self {
        Self::new(span, text, LabelSeverity::Secondary)
    }
}

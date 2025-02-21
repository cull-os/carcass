use std::borrow::Cow;

use crate::{
    ReportSeverity,
    Span,
    into,
};

/// The severity of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSeverity {
    Secondary,
    Primary,
}

impl LabelSeverity {
    /// Returns the applicable style of this label severity in the given report
    /// severity.
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

/// A label for a span.
#[derive(Debug, Clone)]
pub struct Label {
    /// The span.
    pub span: Span,
    /// The label severity.
    pub level: LabelSeverity,
    /// The text that will be displayed at the end of the label.
    pub text: Cow<'static, str>,
}

impl Label {
    /// Creates a new [`Label`].
    #[inline]
    pub fn new(span: impl Into<Span>, text: impl Into<Cow<'static, str>>, level: LabelSeverity) -> Self {
        into!(span, text);

        Self { span, text, level }
    }

    /// Creates a new primary [`Label`].
    #[inline]
    pub fn primary(span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
        Self::new(span, text, LabelSeverity::Primary)
    }

    /// Creates a new secondary [`Label`].
    #[inline]
    pub fn secondary(span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
        Self::new(span, text, LabelSeverity::Secondary)
    }
}

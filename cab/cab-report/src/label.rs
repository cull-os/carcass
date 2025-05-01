use std::borrow::Cow;

use cab_format::style::{
   Color,
   Style,
};
use cab_span::Span;
use cab_util::into;

use crate::ReportSeverity;

/// The severity of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSeverity {
   Secondary,
   Primary,
}

impl LabelSeverity {
   /// Returns the applicable style of this label severity in the given report
   /// severity.
   pub fn style_in(self, severity: ReportSeverity) -> Style {
      use LabelSeverity::{
         Primary,
         Secondary,
      };
      use ReportSeverity::{
         Bug,
         Error,
         Note,
         Warn,
      };

      match (severity, self) {
         (Note, Secondary) => Color::Blue,
         (Note, Primary) => Color::Magenta,

         (Warn, Secondary) => Color::Blue,
         (Warn, Primary) => Color::Yellow,

         (Error, Secondary) => Color::Yellow,
         (Error, Primary) => Color::Red,

         (Bug, Secondary) => Color::Yellow,
         (Bug, Primary) => Color::Red,
      }
      .foreground()
   }
}

/// A label for a span.
#[derive(Debug, Clone)]
pub struct Label {
   /// The span.
   pub span:     Span,
   /// The label severity.
   pub severity: LabelSeverity,
   /// The text that will be displayed at the end of the label.
   pub text:     Cow<'static, str>,
}

impl Label {
   /// Creates a new [`Label`].
   #[inline]
   pub fn new(
      span: impl Into<Span>,
      text: impl Into<Cow<'static, str>>,
      severity: LabelSeverity,
   ) -> Self {
      into!(span, text);

      Self {
         span,
         text,
         severity,
      }
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

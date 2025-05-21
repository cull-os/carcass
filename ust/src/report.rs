use std::{
   borrow::Cow,
   sync,
};

use cab_span::{
   Size,
   Span,
};
use cab_util::into;
use derive_more::Deref;
use smallvec::SmallVec;

use crate::{
   style::{
      Color,
      Style,
   },
   terminal,
};

/// A position in a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
   /// The line number. One indexed.
   pub line:   u32,
   /// The column. One indexed.
   pub column: u32,
}

/// A structure that caches source positions, for efficient line/column lookup.
#[derive(Deref, Debug, Clone, PartialEq, Eq)]
pub struct PositionStr<'a> {
   #[deref]
   content:  &'a str,
   newlines: sync::OnceLock<SmallVec<Size, 16>>,
}

impl<'a> PositionStr<'a> {
   /// Creates a new [`PositionStr`] that hasn't cached anything yet.
   #[must_use]
   pub fn new(content: &'a str) -> Self {
      Self {
         content,
         newlines: sync::OnceLock::new(),
      }
   }

   /// Looks up the line/column of an offset.
   pub fn position(&self, offset: Size) -> Position {
      let newlines = self.newlines.get_or_init(|| {
         self
            .content
            .bytes()
            .enumerate()
            .filter_map(|(index, c)| (c == b'\n').then_some(Size::new(index)))
            .collect()
      });

      let line_index = newlines.partition_point(|&line_offset| offset > line_offset);

      let line_start = if line_index == 0 {
         0
      } else {
         *newlines[line_index - 1] + 1
      };

      Position {
         line: u32::try_from(line_index).expect("line index must fit in u32") + 1,

         // Terminal width, because source code should be monospace everywhere.
         column: u32::try_from(terminal::width(
            &self.content[Span::std(line_start, offset)],
         ))
         .expect("column must fit in u32")
            + 1,
      }
   }

   /// Looks up the start/end line/column of a span.
   pub fn positions(&self, span: Span) -> (Position, Position) {
      (self.position(span.start), self.position(span.end))
   }
}

/// The severity of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSeverity {
   Secondary,
   Primary,
}

impl LabelSeverity {
   /// Returns the applicable style of this label severity in the given report
   /// severity.
   #[must_use]
   pub fn style_in(self, report_severity: ReportSeverity) -> Style {
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

      let color = match (report_severity, self) {
         (Note, Secondary) => Color::Blue,
         (Note, Primary) => Color::Magenta,

         (Warn, Secondary) => Color::Blue,
         (Warn, Primary) => Color::Yellow,

         (Error, Secondary) => Color::Yellow,
         (Error, Primary) => Color::Red,

         (Bug, Secondary) => Color::Yellow,
         (Bug, Primary) => Color::Red,
      };

      if self == Primary {
         color.fg().bold()
      } else {
         color.fg()
      }
   }
}

/// A label for a span.
#[derive(Debug, Clone)]
pub struct Label {
   /// The span.
   pub span:     Span,
   /// The severity of the label.
   pub severity: LabelSeverity,
   /// The text that will be displayed at the end of the label.
   pub text:     Cow<'static, str>,
}

impl Label {
   /// Creates a new [`Label`].
   #[inline]
   pub fn new(
      span: impl Into<Span>,
      severity: LabelSeverity,
      text: impl Into<Cow<'static, str>>,
   ) -> Self {
      into!(span, text);

      Self {
         span,
         severity,
         text,
      }
   }

   /// Creates a new primary [`Label`].
   #[inline]
   pub fn primary(span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      Self::new(span, LabelSeverity::Primary, text)
   }

   /// Creates a new secondary [`Label`].
   #[inline]
   pub fn secondary(span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      Self::new(span, LabelSeverity::Secondary, text)
   }
}

/// The severity of a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PointSeverity {
   Tip,
   Help,
}

impl PointSeverity {
   pub fn style_in(self) -> Style {
      match self {
         PointSeverity::Tip => Color::Cyan,
         PointSeverity::Help => Color::Magenta,
      }
      .fg()
   }
}

/// A spanless label, also known as a point. Displayed at the end of the report.
#[derive(Debug, Clone)]
pub struct Point {
   /// The severity of the point.
   pub severity: PointSeverity,
   /// The text of the point.
   pub text:     Cow<'static, str>,
}

impl Point {
   /// Creates a new [`Point`].
   pub fn new(severity: PointSeverity, text: impl Into<Cow<'static, str>>) -> Self {
      into!(text);

      Self { severity, text }
   }

   /// Creates a tip [`Point`].
   pub fn tip(text: impl Into<Cow<'static, str>>) -> Self {
      Self::new(PointSeverity::Tip, text)
   }

   /// Creates a help [`Point`].
   pub fn help(text: impl Into<Cow<'static, str>>) -> Self {
      Self::new(PointSeverity::Help, text)
   }
}

/// The severity of a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportSeverity {
   Note,
   Warn,
   Error,
   Bug,
}

impl ReportSeverity {
   pub fn style_in(self) -> Style {
      LabelSeverity::Primary.style_in(self)
   }
}

#[derive(Debug, Clone)]
pub struct Report {
   pub severity: ReportSeverity,
   pub title:    Cow<'static, str>,
   pub labels:   SmallVec<Label, 2>,
   pub points:   SmallVec<Point, 2>,
}

impl Report {
   pub fn new(severity: ReportSeverity, title: impl Into<Cow<'static, str>>) -> Self {
      into!(title);

      Self {
         title,
         severity,
         labels: SmallVec::new(),
         points: SmallVec::new(),
      }
   }

   pub fn note(title: impl Into<Cow<'static, str>>) -> Self {
      Self::new(ReportSeverity::Note, title)
   }

   pub fn warn(title: impl Into<Cow<'static, str>>) -> Self {
      Self::new(ReportSeverity::Warn, title)
   }

   pub fn error(title: impl Into<Cow<'static, str>>) -> Self {
      Self::new(ReportSeverity::Error, title)
   }

   pub fn bug(title: impl Into<Cow<'static, str>>) -> Self {
      Self::new(ReportSeverity::Bug, title)
   }

   #[must_use]
   pub fn is_empty(&self) -> bool {
      self.labels.is_empty() && self.points.is_empty()
   }

   pub fn push_label(&mut self, label: Label) {
      self.labels.push(label);
   }

   pub fn push_primary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
      self.labels.push(Label::primary(span, text));
   }

   #[must_use]
   pub fn primary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      self.push_primary(span, text);
      self
   }

   pub fn push_secondary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
      self.labels.push(Label::secondary(span, text));
   }

   #[must_use]
   pub fn secondary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      self.push_secondary(span, text);
      self
   }

   #[must_use]
   pub fn point(mut self, point: Point) -> Self {
      self.points.push(point);
      self
   }

   pub fn push_tip(&mut self, text: impl Into<Cow<'static, str>>) {
      self.points.push(Point::tip(text));
   }

   #[must_use]
   pub fn tip(self, text: impl Into<Cow<'static, str>>) -> Self {
      self.point(Point::tip(text))
   }

   pub fn push_help(&mut self, text: impl Into<Cow<'static, str>>) {
      self.points.push(Point::help(text));
   }

   #[must_use]
   pub fn help(self, text: impl Into<Cow<'static, str>>) -> Self {
      self.point(Point::help(text))
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   #[test]
   fn position() {
      let mut source;

      macro_rules! assert_span {
         (
            $range:expr =>
            $slice:literal,($start_line:literal : $start_column:literal),($end_line:literal : $end_column:literal)
         ) => {
            let range: std::ops::Range<usize> = $range;

            assert_eq!(&source[range.clone()], $slice);

            let (start, end) = source.positions(Span::new(range.start, range.end));

            assert_eq!(start, Position {
               line:   $start_line,
               column: $start_column,
            });
            assert_eq!(end, Position {
               line:   $end_line,
               column: $end_column,
            });
         };
      }

      source = PositionStr::new("foo\nbar");
      assert_span!(0..5 => "foo\nb", (1:1), (2:2));
      assert_span!(0..1 => "f", (1:1), (1:2));

      source = PositionStr::new("foo\næ");
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));

      source = PositionStr::new("foo\næb");
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));
      assert_span!(0..2 => "fo", (1:1), (1:3));
      assert_span!(0..4 => "foo\n", (1:1), (2:1));
      assert_span!(0..6 => "foo\næ", (1:1), (2:2));
      assert_span!(0..7 => "foo\næb", (1:1), (2:3));
      assert_span!(3..7 => "\næb", (1:4), (2:3));
      assert_span!(4..7 => "æb", (2:1), (2:3));
   }
}

mod label;
mod point;
mod position;

use std::{
   borrow::Cow,
   cell::RefCell,
   cmp,
   error,
   fmt::{
      self,
      Write as _,
   },
   iter,
};

use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use yansi::Paint;

pub use self::{
   label::{
      Label,
      LabelSeverity,
   },
   point::Point,
   position::Position,
};
use crate::{
   IntoSize,
   Size,
   Span,
   dedent,
   indent,
   into,
   wrapln,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReportSeverity {
   Note,
   Warn,
   Error,
   Bug,
}

impl ReportSeverity {
   pub fn header(self) -> yansi::Painted<&'static str> {
      match self {
         ReportSeverity::Note => "note:",
         ReportSeverity::Warn => "warn:",
         ReportSeverity::Error => "error:",
         ReportSeverity::Bug => "bug:",
      }
      .paint(LabelSeverity::Primary.style_in(self))
      .bold()
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

   pub fn is_empty(&self) -> bool {
      self.labels.is_empty() && self.points.is_empty()
   }

   pub fn push_label(&mut self, label: Label) {
      self.labels.push(label)
   }

   pub fn push_primary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
      self.labels.push(Label::primary(span, text));
   }

   pub fn primary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      self.push_primary(span, text);
      self
   }

   pub fn push_secondary(&mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) {
      self.labels.push(Label::secondary(span, text));
   }

   pub fn secondary(mut self, span: impl Into<Span>, text: impl Into<Cow<'static, str>>) -> Self {
      self.push_secondary(span, text);
      self
   }

   pub fn point(mut self, point: Point) -> Self {
      self.points.push(point);
      self
   }

   pub fn push_tip(&mut self, text: impl Into<Cow<'static, str>>) {
      self.points.push(Point::tip(text));
   }

   pub fn tip(self, text: impl Into<Cow<'static, str>>) -> Self {
      self.point(Point::tip(text))
   }

   pub fn push_help(&mut self, text: impl Into<Cow<'static, str>>) {
      self.points.push(Point::help(text));
   }

   pub fn help(self, text: impl Into<Cow<'static, str>>) -> Self {
      self.point(Point::help(text))
   }

   pub fn with<Location: fmt::Display>(
      self,
      location: Location,
      source: &str,
   ) -> impl error::Error {
      ReportDisplay::from(self, source, location)
   }
}

fn number_width(number: u32) -> usize {
   if number == 0 {
      1
   } else {
      (number as f64).log10() as usize + 1
   }
}

fn is_emoji(s: &str) -> bool {
   !s.is_ascii() && s.chars().any(unic_emoji_char::is_emoji)
}

pub fn width(s: &str) -> usize {
   s.graphemes(true)
      .map(|grapheme| {
         match grapheme {
            "\t" => 4,
            s if is_emoji(s) => 2,
            s => s.width(),
         }
      })
      .sum()
}

fn extend_to_line_boundaries(source: &str, mut span: Span) -> Span {
   while *span.start > 0
      && source
         .as_bytes()
         .get(*span.start as usize - 1)
         .is_some_and(|&c| c != b'\n')
   {
      span.start -= 1u32;
   }

   while source
      .as_bytes()
      .get(*span.end as usize)
      .is_some_and(|&c| c != b'\n')
   {
      span.end += 1u32;
   }

   span
}

/// Given a list of spans which refer to the given content and their associated
/// severities (primary and secondary), resolves the colors for every part,
/// giving the primary color precedence over the secondary color in an overlap.
fn resolve_style<'a>(
   content: &'a str,
   styles: &'a [LineStyle],
   severity: ReportSeverity,
) -> impl Iterator<Item = yansi::Painted<&'a str>> + 'a {
   gen move {
      let mut content_offset = Size::new(0u32);
      let mut style_offset: usize = 0;

      while content_offset < content.len().into() {
         let current_style =
            styles[style_offset..]
               .iter()
               .copied()
               .enumerate()
               .find(|(_, style)| {
                  style.span.start <= content_offset && content_offset < style.span.end
               });

         match current_style {
            Some((style_offset_diff, style)) => {
               style_offset += style_offset_diff;

               let contained_primary = (style.severity == LabelSeverity::Secondary)
                  .then(|| {
                     styles[style_offset..]
                        .iter()
                        .copied()
                        .enumerate()
                        .take_while(|(_, other)| other.span.start <= style.span.end)
                        .find(|(_, other)| {
                           other.severity == LabelSeverity::Primary
                              && other.span.start > content_offset
                        })
                  })
                  .flatten();

               match contained_primary {
                  Some((style_offset_diff, contained_style)) => {
                     style_offset += style_offset_diff;

                     yield content[Span::std(content_offset, contained_style.span.start)]
                        .paint(style.severity.style_in(severity));

                     yield content[contained_style.span.as_std()]
                        .paint(contained_style.severity.style_in(severity));

                     yield content[Span::std(contained_style.span.end, style.span.end)]
                        .paint(style.severity.style_in(severity));
                  },

                  None => {
                     yield content[Span::std(content_offset, style.span.end)]
                        .paint(style.severity.style_in(severity));
                  },
               }

               content_offset = style.span.end;
            },

            None => {
               let (relative_offset, next_offset) = styles[style_offset..]
                  .iter()
                  .enumerate()
                  .filter(|(_, style)| style.span.start > content_offset)
                  .map(|(relative_offset, style)| (relative_offset, style.span.start))
                  .next()
                  .unwrap_or((styles.len() - style_offset, content.len().into()));

               style_offset += relative_offset;

               yield (&content[Span::std(content_offset, next_offset)]).new();
               content_offset = next_offset;
            },
         }
      }
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineStrikeId(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineStrikeStatus {
   Start,
   Continue,
   End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineStrike {
   id:       LineStrikeId,
   status:   LineStrikeStatus,
   severity: LabelSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineStyle {
   span:     Span,
   severity: LabelSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineLabelSpan {
   /// Guaranteed to start at 0.
   UpTo(Span),
   Inline(Span),
}

impl LineLabelSpan {
   fn start(self) -> Option<Size> {
      match self {
         LineLabelSpan::UpTo(_) => None,
         LineLabelSpan::Inline(span) => Some(span.start),
      }
   }

   fn end(self) -> Size {
      match self {
         LineLabelSpan::UpTo(span) => span.end,
         LineLabelSpan::Inline(span) => span.end,
      }
   }

   fn is_empty(self) -> bool {
      self.start().is_some_and(|start| start == self.end())
   }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineLabel {
   span:     LineLabelSpan,
   text:     Cow<'static, str>,
   severity: LabelSeverity,
}

#[derive(Debug, Clone)]
struct Line {
   number: u32,

   strikes: SmallVec<LineStrike, 3>,

   content: String,
   styles:  SmallVec<LineStyle, 4>,

   labels: SmallVec<LineLabel, 2>,
}

#[derive(Clone)]
struct ReportDisplay<Location: fmt::Display> {
   severity: ReportSeverity,
   title:    Cow<'static, str>,

   location: Location,

   lines: SmallVec<Line, 10>,

   points: SmallVec<Point, 2>,
}

const RIGHT_TO_BOTTOM: char = '┏';
const TOP_TO_BOTTOM: char = '┃';
const TOP_TO_BOTTOM_PARTIAL: char = '┇';
const DOT: char = '·';
const TOP_TO_RIGHT: char = '┗';
const LEFT_TO_RIGHT: char = '━';
const LEFT_TO_TOP_BOTTOM: char = '┫';

const TOP_TO_BOTTOM_LEFT: char = '▏';
const TOP_LEFT_TO_RIGHT: char = '╲';
const TOP_TO_BOTTOM_RIGHT: char = '▕';

const STYLE_GUTTER: yansi::Style = yansi::Style::new().blue();
const STYLE_HEADER_PATH: yansi::Style = yansi::Style::new().green();
const STYLE_HEADER_POSITION: yansi::Style = yansi::Style::new().blue();

impl<Location: fmt::Display> fmt::Display for ReportDisplay<Location> {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      {
         // INDENT: "<note|warn|error|bug>: "
         indent!(writer, header = self.severity.header());

         wrapln(writer, [self.title.as_ref().bold()])?;
      }

      let line_number_width = self
         .lines
         .last()
         .map_or(0, |line| number_width(line.number));

      // INDENT: "123 | "
      let line_number = RefCell::new(None::<u32>);
      let line_number_should_write = RefCell::new(false);
      let line_number_previous = RefCell::new(None::<u32>);
      indent!(
         writer,
         line_number_width + 3,
         with = |writer: &mut dyn fmt::Write| {
            let Some(line_number) = *line_number.borrow() else {
               return Ok(0);
            };

            let mut line_number_previous = line_number_previous.borrow_mut();

            STYLE_GUTTER.fmt_prefix(writer)?;
            match () {
               // Don't write the current line number, just print spaces instead.
               _ if !*line_number_should_write.borrow() => {
                  write!(writer, "{:>line_number_width$}", "")?;
               },

               // Continuation line. Use dots instead of the number.
               _ if *line_number_previous == Some(line_number) => {
                  let dot_width = number_width(line_number);
                  let space_width = line_number_width - dot_width;

                  write!(writer, "{:>space_width$}", "")?;

                  for _ in 0..dot_width {
                     write!(writer, "{DOT}")?;
                  }
               },

               // New line, but not right after the previous line. Also known as a non-incremental
               // jump.
               _ if line_number_previous
                  .is_some_and(|line_number_previous| line_number > line_number_previous + 1) =>
               {
                  writeln!(
                     writer,
                     "{:>line_number_width$} {TOP_TO_BOTTOM_PARTIAL} ",
                     ""
                  )?;
                  write!(writer, "{line_number:>line_number_width$}")?;
               },

               // New line.
               _ => {
                  write!(writer, "{line_number:>line_number_width$}")?;
               },
            }

            write!(writer, " {TOP_TO_BOTTOM} ")?;
            STYLE_GUTTER.fmt_suffix(writer)?;

            line_number_previous.replace(line_number);
            Ok(line_number_width + 3)
         }
      );

      if let Some(line) = self.lines.first() {
         // DEDENT: "| "
         dedent!(writer, 2);

         // INDENT: "┏━━━ ".
         indent!(
            writer,
            header =
               const_str::concat!(RIGHT_TO_BOTTOM, LEFT_TO_RIGHT, LEFT_TO_RIGHT, LEFT_TO_RIGHT)
                  .paint(STYLE_GUTTER)
         );

         STYLE_HEADER_PATH.fmt_prefix(writer)?;
         write!(writer, "{location}", location = self.location)?;
         STYLE_HEADER_PATH.fmt_suffix(writer)?;

         let line_number = line.number.paint(STYLE_HEADER_POSITION);
         let column_number = *line.styles.first().unwrap().span.start + 1;
         let column_number = column_number.paint(STYLE_HEADER_POSITION);
         writeln!(writer, ":{line_number}:{column_number}")?;
      }

      let strike_prefix_width = self
         .lines
         .iter()
         .map(|line| line.strikes.len())
         .max()
         .unwrap_or(0);

      {
         // INDENT: "<strike-prefix> "
         let strike_prefix = RefCell::new(SmallVec::<_, 3>::from_iter(iter::repeat_n(
            None::<LineStrike>,
            strike_prefix_width,
         )));
         indent!(
            writer,
            strike_prefix_width + 1,
            with = |writer: &mut dyn fmt::Write| {
               const STRIKE_OVERRIDE_DEFAULT: yansi::Painted<&char> = yansi::Painted::new(&' ');

               let mut strike_override = None::<yansi::Painted<&char>>;

               for slot in &*strike_prefix.borrow() {
                  let Some(strike) = *slot else {
                     write!(
                        writer,
                        "{symbol}",
                        symbol = strike_override.unwrap_or(STRIKE_OVERRIDE_DEFAULT)
                     )?;
                     continue;
                  };

                  match strike.status {
                     LineStrikeStatus::Start => {
                        write!(
                           writer,
                           "{symbol}",
                           symbol = RIGHT_TO_BOTTOM.paint(self.style(strike.severity))
                        )?;

                        strike_override = Some(LEFT_TO_RIGHT.paint(self.style(strike.severity)));
                     },

                     LineStrikeStatus::Continue | LineStrikeStatus::End
                        if let Some(strike) = strike_override =>
                     {
                        write!(writer, "{strike}")?;
                     },

                     LineStrikeStatus::Continue | LineStrikeStatus::End => {
                        write!(
                           writer,
                           "{symbol}",
                           symbol = TOP_TO_BOTTOM.paint(self.style(strike.severity))
                        )?;
                     },
                  }
               }

               write!(
                  writer,
                  "{symbol}",
                  symbol = strike_override.unwrap_or(STRIKE_OVERRIDE_DEFAULT)
               )?;

               Ok(strike_prefix_width + 1)
            }
         );

         for (line_index, line) in self.lines.iter().enumerate() {
            line_number.borrow_mut().replace(line.number);

            // Write an empty line at the start.
            if line_index == 0 {
               *line_number_should_write.borrow_mut() = false;

               writer.write_indent()?;
               writeln!(writer)?;

               *line_number_previous.borrow_mut() = None;
            }

            // Patch strike prefix and keep track of positions of strikes with their IDs.
            {
               let mut strike_prefix = strike_prefix.borrow_mut();

               for strike_new @ LineStrike { id, .. } in line.strikes.iter().copied() {
                  match strike_prefix
                     .iter_mut()
                     .flatten()
                     .find(|strike| strike.id == id)
                  {
                     Some(strike) => *strike = strike_new,

                     None => {
                        strike_prefix
                           .iter_mut()
                           .find(|slot| slot.is_none())
                           .unwrap()
                           .replace(strike_new);
                     },
                  }
               }
            }

            // Write the line.
            {
               *line_number_should_write.borrow_mut() = true;

               // Explicitly write the indent because the line may be empty.
               writer.write_indent()?;
               wrapln(
                  writer,
                  resolve_style(&line.content, &line.styles, self.severity),
               )?;

               *line_number_should_write.borrow_mut() = false;
            }

            // Write the line labels.
            // Reverse, because we want to print the labels that end the last first.
            for (label_index, label) in line.labels.iter().enumerate().rev() {
               // HACK: wrapln may split the current line into multiple
               // lines, so the label pointer may be too far left.
               // Just max it to 60 for now.
               let span_start = label.span.start().min(Some(60u32.into()));
               let span_end = label.span.end().min(60u32.into());

               // DEDENT: "<strike-prefix> "
               dedent!(writer);

               match label.span {
                  LineLabelSpan::UpTo(_) => {
                     let (top_to_right_index, top_to_right) = strike_prefix
                        .borrow()
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(index, strike)| {
                           match strike {
                              Some(strike) if strike.status == LineStrikeStatus::End => {
                                 Some((index, *strike))
                              },

                              _ => None,
                           }
                        })
                        .unwrap();

                     assert_eq!(top_to_right.severity, label.severity);

                     // INDENT: "<strike-prefix>"
                     let mut wrote = false;
                     indent!(
                        writer,
                        strike_prefix_width,
                        with = |writer: &mut dyn fmt::Write| {
                           // Write all strikes up to the index of the one we are going to redirect
                           // to the right.
                           for slot in strike_prefix.borrow().iter().take(top_to_right_index) {
                              write!(
                                 writer,
                                 "{symbol}",
                                 symbol = match slot {
                                    Some(strike) =>
                                       TOP_TO_BOTTOM.paint(self.style(strike.severity)),
                                    None => (&' ').new(),
                                 }
                              )?;
                           }

                           if wrote {
                              return Ok(top_to_right_index);
                           }

                           write!(
                              writer,
                              "{symbol}",
                              symbol = TOP_TO_RIGHT.paint(self.style(top_to_right.severity))
                           )?;

                           for _ in 0..strike_prefix_width - top_to_right_index - 1 {
                              write!(
                                 writer,
                                 "{symbol}",
                                 symbol = LEFT_TO_RIGHT.paint(self.style(top_to_right.severity))
                              )?;
                           }

                           wrote = true;
                           Ok(strike_prefix_width)
                        }
                     );

                     // INDENT: "<left-to-right><left-to-bottom>"
                     // INDENT: "               <top--to-bottom>"
                     let mut wrote = false;
                     indent!(
                        writer,
                        // + 1 because the span is zero-indexed and we didn't indent the space
                        //   after <strike-prefix> before.
                        //
                        // + 1 because we want a space after the <top-to-bottom>.
                        *span_end + 2,
                        with = |writer: &mut dyn fmt::Write| {
                           for index in 0..*span_end {
                              write!(
                                 writer,
                                 "{symbol}",
                                 symbol = match () {
                                    // If there is a label on the current line after this label that
                                    // has a start or end at the
                                    // current index, write it instead of out <left-to-right>
                                    _ if let Some(label) =
                                       line.labels[..label_index].iter().rev().find(|label| {
                                          *label.span.end() == index && !label.span.is_empty()
                                             || label
                                                .span
                                                .start()
                                                .is_some_and(|start| *start + 1 == index)
                                       }) =>
                                    {
                                       if label.span.is_empty() {
                                          TOP_TO_BOTTOM_LEFT.paint(self.style(label.severity))
                                       } else {
                                          TOP_TO_BOTTOM.paint(self.style(label.severity))
                                       }
                                    },

                                    _ if !wrote =>
                                       LEFT_TO_RIGHT.paint(self.style(top_to_right.severity)),

                                    _ => (&' ').new(),
                                 }
                              )?;
                           }

                           write!(
                              writer,
                              "{symbol}",
                              symbol = match () {
                                 _ if !wrote => LEFT_TO_TOP_BOTTOM,
                                 _ => TOP_TO_BOTTOM,
                              }
                              .paint(self.style(top_to_right.severity))
                           )?;

                           wrote = true;
                           strike_prefix.borrow_mut()[top_to_right_index] = None;
                           Ok(*span_end + 1)
                        }
                     );

                     wrapln(writer, [label
                        .text
                        .as_ref()
                        .paint(self.style(top_to_right.severity))])?;
                  },

                  LineLabelSpan::Inline(_) => {
                     let span_start = span_start.unwrap();

                     // INDENT: "<strike-prefix> "
                     indent!(
                        writer,
                        strike_prefix_width + 1,
                        with = |writer: &mut dyn fmt::Write| {
                           for slot in &*strike_prefix.borrow() {
                              write!(
                                 writer,
                                 "{symbol}",
                                 symbol = match slot {
                                    Some(strike) =>
                                       TOP_TO_BOTTOM.paint(self.style(strike.severity)),
                                    None => (&' ').new(),
                                 }
                              )?;
                           }

                           Ok(strike_prefix_width)
                        }
                     );

                     // INDENT: "               <top-to-right><left-to-right><left-to-bottom> "
                     // INDENT: "                                            <top--to-bottom> "
                     let mut wrote = false;
                     indent!(
                        writer,
                        // + 1 for extra space.
                        // + 1 if the label is zero-width. The <top-left-to-right> will be placed
                        //   after the span.
                        *span_end + if span_start == span_end { 1 } else { 0 } + 1,
                        with = |writer: &mut dyn fmt::Write| {
                           for index in 0..*span_end - if span_start == span_end { 0 } else { 1 } {
                              write!(
                                 writer,
                                 "{symbol}",
                                 symbol = match () {
                                    _ if index == *span_start =>
                                       TOP_TO_RIGHT.paint(self.style(label.severity)),

                                    _ if let Some(label) =
                                       line.labels[..label_index].iter().rev().find(|label| {
                                          *label.span.end() == index + 1 && !label.span.is_empty()
                                             || label
                                                .span
                                                .start()
                                                .is_some_and(|start| *start == index)
                                       }) =>
                                    {
                                       if label.span.is_empty() {
                                          TOP_TO_BOTTOM_LEFT.paint(self.style(label.severity))
                                       } else {
                                          TOP_TO_BOTTOM.paint(self.style(label.severity))
                                       }
                                    },

                                    _ if !wrote && index > *span_start => {
                                       LEFT_TO_RIGHT.paint(self.style(label.severity))
                                    },

                                    _ => (&' ').new(),
                                 }
                              )?;
                           }

                           write!(
                              writer,
                              "{symbol}",
                              symbol = match *span_end - *span_start {
                                 0 if wrote => TOP_TO_BOTTOM_RIGHT,
                                 _ if wrote => TOP_TO_BOTTOM,

                                 0 => TOP_LEFT_TO_RIGHT,
                                 1 => TOP_TO_BOTTOM,

                                 _ => LEFT_TO_TOP_BOTTOM,
                              }
                              .paint(self.style(label.severity))
                           )?;

                           wrote = true;
                           Ok(*span_end + if span_start == span_end { 1 } else { 0 })
                        }
                     );

                     wrapln(writer, [label
                        .text
                        .as_ref()
                        .paint(self.style(label.severity))])?;
                  },
               }
            }
         }
      }

      // Write the points.
      {
         if !self.points.is_empty() {
            writer.write_indent()?;
            writeln!(writer)?;
         }

         // DEDENT: "| "
         dedent!(writer, 2);

         for point in &self.points {
            // INDENT: "= "
            indent!(writer, header = "=".paint(STYLE_GUTTER));

            // INDENT: "<tip|help|...>: "
            indent!(writer, header = &point.title);

            wrapln(writer, [point.text.as_ref().new()])?;
         }
      }

      Ok(())
   }
}

impl<Location: fmt::Display> fmt::Debug for ReportDisplay<Location> {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      fmt::Display::fmt(self, writer)
   }
}

impl<Location: fmt::Display> error::Error for ReportDisplay<Location> {}

impl<Location: fmt::Display> ReportDisplay<Location> {
   fn from(report: Report, source: &str, location: Location) -> Self {
      let mut labels: SmallVec<_, 2> = report
         .labels
         .into_iter()
         .map(|label| (Position::of(label.span, source), label))
         .collect();

      // Sort by line, and when labels are on the same line, sort by column. The one
      // that ends the last will be the last.
      labels.sort_by(|((a_start, a_end), _), ((b_start, b_end), _)| {
         a_start
            .line
            .cmp(&b_start.line)
            .then_with(|| a_end.column.cmp(&b_end.column))
      });

      let mut lines = SmallVec::<Line, 10>::new();

      'labels: for (label_index, ((label_start, label_end), label)) in
         labels.into_iter().enumerate()
      {
         let label_span_extended = extend_to_line_boundaries(source, label.span);

         for (line_number, line_content) in
            (label_start.line..).zip(source[label_span_extended.as_std()].split('\n'))
         {
            let line = match lines.iter_mut().find(|line| line.number == line_number) {
               Some(item) => item,

               None => {
                  lines.push(Line {
                     number: line_number,

                     strikes: SmallVec::new(),

                     content: line_content.to_owned(),
                     styles:  SmallVec::new(),

                     labels: SmallVec::new(),
                  });

                  lines.last_mut().expect("line was pushed")
               },
            };

            let line_is_first = line_number == label_start.line;
            let line_is_last = line_number == label_end.line;

            // Not in a single line label.
            if !(line_is_first && line_is_last) {
               line.strikes.push(LineStrike {
                  id: LineStrikeId(
                     label_index
                        .try_into()
                        .expect("overlapping label count must not exceed u8::MAX"),
                  ),

                  status: match () {
                     _ if line_is_first => LineStrikeStatus::Start,
                     _ if line_is_last => LineStrikeStatus::End,
                     _ => LineStrikeStatus::Continue,
                  },

                  severity: label.severity,
               });
            }

            match (line_is_first, line_is_last) {
               // Single line label.
               (true, true) => {
                  let base = label_span_extended.start;

                  let Span { start, end } = label.span;
                  let span = Span::new(start - base, end - base);

                  line.styles.push(LineStyle {
                     span,
                     severity: label.severity,
                  });

                  let up_to_start_width = width(&line_content[..*span.start as usize]);
                  let label_width = width(&line_content[span.as_std()]);

                  line.labels.push(LineLabel {
                     span:     LineLabelSpan::Inline(Span::at(up_to_start_width, label_width)),
                     text:     label.text,
                     severity: label.severity,
                  });
                  continue 'labels;
               },

               // Multiline label's first line.
               (true, false) => {
                  let base = label_span_extended.start;

                  let Span { start, .. } = label.span;
                  let end = source[*start as usize..]
                     .find('\n')
                     .map_or(source.size(), |index| start + index);

                  let span = Span::new(start - base, end - base);

                  line.styles.push(LineStyle {
                     span,
                     severity: label.severity,
                  })
               },

               // Multiline label's intermediary line.
               (false, false) => {
                  line.styles.push(LineStyle {
                     span:     Span::up_to(line.content.len()),
                     severity: label.severity,
                  });
               },

               // Multiline label's last line.
               (false, true) => {
                  let roof = label_span_extended.end;

                  // Line being:
                  // <<<pointed-at>>><<<rest>>>
                  //                 ^^^^^^^^^^ length of this
                  let rest = roof - label.span.end;

                  let end = line.content.as_str().size() - rest;

                  let span = Span::up_to(end);

                  line.styles.push(LineStyle {
                     span,
                     severity: label.severity,
                  });

                  let up_to_end_width = width(&line_content[..*end as usize]);

                  line.labels.push(LineLabel {
                     span:     LineLabelSpan::UpTo(Span::up_to(up_to_end_width)),
                     text:     label.text,
                     severity: label.severity,
                  });
                  continue 'labels;
               },
            }
         }
      }

      for line in &mut lines {
         line.styles.sort_by(|a_style, b_style| {
            match (
               a_style.span.start.cmp(&b_style.span.start),
               a_style.severity,
               b_style.severity,
            ) {
               (cmp::Ordering::Equal, LabelSeverity::Primary, LabelSeverity::Secondary) => {
                  cmp::Ordering::Less
               },

               (cmp::Ordering::Equal, LabelSeverity::Secondary, LabelSeverity::Primary) => {
                  cmp::Ordering::Greater
               },

               (ordering, ..) => ordering,
            }
         });

         line.labels.sort_by_key(|style| {
            // Empty labels are printed offset one column to the right, so treat them like
            // it.
            style.span.end() + if style.span.is_empty() { 1u32 } else { 0u32 }
         });
      }

      Self {
         severity: report.severity,
         title: report.title,

         location,

         lines,

         points: report.points,
      }
   }

   fn style(&self, severity: LabelSeverity) -> yansi::Style {
      severity.style_in(self.severity)
   }
}

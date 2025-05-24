use std::{
   borrow::Cow,
   cell::RefCell,
   cmp,
   fmt::{
      self,
      Write as _,
   },
   iter,
   num::NonZeroUsize,
   ops::Add as _,
   os,
};

use cab_span::{
   IntoSize as _,
   Size,
   Span,
};
use cab_util::{
   as_,
   borrow_mut,
   into_iter,
   unwrap,
};
use itertools::Itertools as _;
use num::traits::AsPrimitive;
use scopeguard::{
   ScopeGuard,
   guard,
};
use smallvec::SmallVec;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
   Display,
   IndentWith,
   IntoWidth,
   SPACES,
   Write,
   report,
   style::{
      self,
      StyledExt,
   },
};

/// Calculates the width of the number when formatted with the default
/// formatter.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn number_width(number: impl AsPrimitive<f64>) -> usize {
   as_!(number);

   if number == 0.0 {
      1
   } else {
      number.log10() as usize + 1
   }
}

/// Calculates the width of the number when formatted with the hex formatter.
///
/// Width does not include `0x` prefix, so beware.
#[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn number_hex_width(number: impl AsPrimitive<f64>) -> usize {
   as_!(number);

   if number == 0.0 {
      1
   } else {
      number.log(16.0) as usize + 1
   }
}

fn is_emoji(s: &str) -> bool {
   !s.is_ascii() && s.chars().any(unic_emoji_char::is_emoji)
}

/// Calculates the width of the string on a best-effort basis.
#[must_use]
pub fn width(s: &str) -> usize {
   s.graphemes(true)
      .map(|grapheme| {
         match grapheme {
            "\t" => 4,
            s if is_emoji(s) => 2,
            #[expect(clippy::disallowed_methods)]
            s => unicode_width::UnicodeWidthStr::width(s),
         }
      })
      .sum::<usize>()
}

/// [`wrap`], but with a newline before the text.
pub fn lnwrap<'a, W: Write>(
   writer: &mut W,
   parts: impl IntoIterator<Item = style::Styled<&'a str>>,
) -> fmt::Result {
   writer.write_char('\n')?;
   wrap(writer, parts)
}

/// Writes the given iterator of colored words into the writer, splicing and
/// wrapping at the max width.
pub fn wrap<'a, W: Write>(
   writer: &mut W,
   parts: impl IntoIterator<Item = style::Styled<&'a str>>,
) -> fmt::Result {
   use None as Newline;
   use Some as Word;

   let mut parts = parts
      .into_iter()
      .flat_map(|part| {
         Iterator::intersperse(
            part
               .value
               .split('\n')
               .map(move |word| Word(word.style(part.style))),
            Newline,
         )
      })
      .peekable();

   while parts.peek().is_some() {
      wrap_line(
         writer,
         parts
            .by_ref()
            .take_while_inclusive(|part| matches!(*part, Word(_)))
            .map(|part| {
               match part {
                  Word(word) => word,
                  Newline => "\n".styled(),
               }
            }),
      )?;
   }

   Ok(())
}

fn wrap_line<'a, W: Write>(
   writer: &mut W,
   parts: impl IntoIterator<Item = style::Styled<&'a str>>,
) -> fmt::Result {
   const WIDTH_NEEDED: NonZeroUsize = NonZeroUsize::new(8).unwrap();

   use None as Space;
   use Some as Word;

   into_iter!(parts);

   let width_start = writer.width();

   let width_max = if width_start + WIDTH_NEEDED.get() <= writer.width_max() {
      writer.width_max()
   } else {
      // If we can't even write WIDTH_NEEDED amount just assume the width is
      // double the worst case width.
      (writer.width_max() + WIDTH_NEEDED.get()) * 2
   };

   let mut parts = parts
      .flat_map(|part| {
         Iterator::intersperse(
            part
               .value
               .split(' ')
               .map(move |word| Word(word.style(part.style))),
            Space,
         )
      })
      .peekable();

   while let Some(part) = parts.peek_mut() {
      let Word(word) = part.as_mut() else {
         if writer.width() != 0 && writer.width() < width_max {
            writer.write_char(' ')?;
         }

         parts.next();
         continue;
      };

      let word_width = width(word.value);

      // Word fits in current line.
      if writer.width() + word_width <= width_max {
         writer.write_styled(word)?;

         parts.next();
         continue;
      }

      // Word fits in the next line.
      if width_start + word_width <= width_max {
         writer.write_char('\n')?;
         writer.write_styled(word)?;

         parts.next();
         continue;
      }

      // Word doesn't fit in the next line.
      let width_remainder = width_max - writer.width();

      let split_index = word
         .value
         .grapheme_indices(true)
         .scan(0, |width, state @ (_, grapheme)| {
            *width += self::width(grapheme);
            Some((*width, state))
         })
         .find_map(|(width, (split_index, _))| (width > width_remainder).then_some(split_index))
         .unwrap();

      let (word_this, word_rest) = word.value.split_at(split_index);

      word.value = word_this;
      writer.write_styled(word)?;

      word.value = word_rest;
   }

   Ok(())
}

const STYLE_RESET: &str = "\x1B[0m";

const STYLE_GUTTER: style::Style = style::Style::new().blue();
const STYLE_HEADER_PATH: style::Style = style::Style::new().green();
const STYLE_HEADER_POSITION: style::Style = style::Style::new().blue();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StyleColorVariant {
   Fg,
   Bg,
}

fn style_color_fg(color: style::Color) -> &'static str {
   match color {
      style::Color::Primary => "39",
      style::Color::Fixed(_) | style::Color::Rgb(..) => "38",
      style::Color::Black => "30",
      style::Color::Red => "31",
      style::Color::Green => "32",
      style::Color::Yellow => "33",
      style::Color::Blue => "34",
      style::Color::Magenta => "35",
      style::Color::Cyan => "36",
      style::Color::White => "37",
      style::Color::BrightBlack => "90",
      style::Color::BrightRed => "91",
      style::Color::BrightGreen => "92",
      style::Color::BrightYellow => "93",
      style::Color::BrightBlue => "94",
      style::Color::BrightMagenta => "95",
      style::Color::BrightCyan => "96",
      style::Color::BrightWhite => "97",
   }
}

fn style_color_bg(color: style::Color) -> &'static str {
   match color {
      style::Color::Primary => "49",
      style::Color::Fixed(_) | style::Color::Rgb(..) => "48",
      style::Color::Black => "40",
      style::Color::Red => "41",
      style::Color::Green => "42",
      style::Color::Yellow => "43",
      style::Color::Blue => "44",
      style::Color::Magenta => "45",
      style::Color::Cyan => "46",
      style::Color::White => "47",
      style::Color::BrightBlack => "100",
      style::Color::BrightRed => "101",
      style::Color::BrightGreen => "102",
      style::Color::BrightYellow => "103",
      style::Color::BrightBlue => "104",
      style::Color::BrightMagenta => "105",
      style::Color::BrightCyan => "106",
      style::Color::BrightWhite => "107",
   }
}

fn style_attr(attr: style::Attr) -> &'static str {
   match attr {
      style::Attr::Bold => "1",
      style::Attr::Dim => "2",
      style::Attr::Italic => "3",
      style::Attr::Underline => "4",
      style::Attr::Blink => "5",
      style::Attr::RapidBlink => "6",
      style::Attr::Invert => "7",
      style::Attr::Conceal => "8",
      style::Attr::Strike => "9",
   }
}

fn style_unattr(attr: style::Attr) -> &'static str {
   match attr {
      style::Attr::Bold => "22",
      style::Attr::Dim => "22",
      style::Attr::Italic => "23",
      style::Attr::Underline => "24",
      style::Attr::Blink => "25",
      style::Attr::RapidBlink => "25",
      style::Attr::Invert => "27",
      style::Attr::Conceal => "28",
      style::Attr::Strike => "29",
   }
}

fn write_style_color(
   writer: &mut impl fmt::Write,
   color: style::Color,
   variant: StyleColorVariant,
) -> fmt::Result {
   writer.write_str(match variant {
      StyleColorVariant::Fg => style_color_fg(color),
      StyleColorVariant::Bg => style_color_bg(color),
   })?;

   match color {
      style::Color::Fixed(num) => {
         let mut buffer = itoa::Buffer::new();

         writer.write_str(";5;")?;
         writer.write_str(buffer.format(num))
      },

      style::Color::Rgb(r, g, b) => {
         let mut buffer = itoa::Buffer::new();

         writer.write_str(";2;")?;
         writer.write_str(buffer.format(r))?;
         writer.write_str(";")?;
         writer.write_str(buffer.format(g))?;
         writer.write_str(";")?;
         writer.write_str(buffer.format(b))
      },

      _ => Ok(()),
   }
}

fn extend_to_line_boundaries(source: &str, mut span: Span) -> Span {
   while *span.start > 0
      && source
         .as_bytes()
         .get(*span.start as usize - 1)
         .is_some_and(|&c| c != b'\n')
   {
      span.start -= 1_u32;
   }

   while source
      .as_bytes()
      .get(*span.end as usize)
      .is_some_and(|&c| c != b'\n')
   {
      span.end += 1_u32;
   }

   span
}

/// Given a list of spans which refer to the given content and their associated
/// severities (primary and secondary), resolves the colors for every part,
/// giving the primary color precedence over the secondary color in an overlap.
fn resolve_style<'a>(
   content: &'a str,
   styles: &'a [LineStyle],
   severity: report::ReportSeverity,
) -> impl Iterator<Item = style::Styled<&'a str>> + 'a {
   gen move {
      let mut content_offset = Size::new(0_u32);
      let mut style_offset: usize = 0;

      while content_offset < content.len().into() {
         let current_style =
            styles[style_offset..]
               .iter()
               .copied()
               .enumerate()
               .find(|&(_, style)| {
                  style.span.start <= content_offset && content_offset < style.span.end
               });

         match current_style {
            Some((style_offset_diff, style)) => {
               style_offset += style_offset_diff;

               let contained_primary = (style.severity == report::LabelSeverity::Secondary)
                  .then(|| {
                     styles[style_offset..]
                        .iter()
                        .copied()
                        .enumerate()
                        .take_while(|&(_, other)| other.span.start <= style.span.end)
                        .find(|&(_, other)| {
                           other.severity == report::LabelSeverity::Primary
                              && other.span.start > content_offset
                        })
                  })
                  .flatten();

               match contained_primary {
                  Some((style_offset_diff, contained_style)) => {
                     style_offset += style_offset_diff;

                     yield content[Span::std(content_offset, contained_style.span.start)]
                        .style(style.severity.style_in(severity));

                     yield content[contained_style.span.into_std()]
                        .style(contained_style.severity.style_in(severity));

                     yield content[Span::std(contained_style.span.end, style.span.end)]
                        .style(style.severity.style_in(severity));
                  },

                  None => {
                     yield content[Span::std(content_offset, style.span.end)]
                        .style(style.severity.style_in(severity));
                  },
               }

               content_offset = style.span.end;
            },

            None => {
               let (relative_offset, next_offset) = styles[style_offset..]
                  .iter()
                  .enumerate()
                  .filter(|&(_, style)| style.span.start > content_offset)
                  .map(|(relative_offset, style)| (relative_offset, style.span.start))
                  .next()
                  .unwrap_or((styles.len() - style_offset, content.len().into()));

               style_offset += relative_offset;

               yield content[Span::std(content_offset, next_offset)].styled();
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
   severity: report::LabelSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineStyle {
   span:     Span,
   severity: report::LabelSeverity,
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
   severity: report::LabelSeverity,
}

#[derive(Debug, Clone)]
struct Line {
   number: u32,

   strikes: SmallVec<LineStrike, 2>,

   content: String,
   styles:  SmallVec<LineStyle, 4>,

   labels: SmallVec<LineLabel, 2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndentPlace {
   Start,
   Middle,
   End,
}

struct Writer<W: fmt::Write> {
   inner: W,

   style: style::Style,

   indents: SmallVec<(u8, IndentWith<Self>), 4>,
   place:   IndentPlace,

   width:     usize,
   width_max: usize,
}

impl<W: fmt::Write> Writer<W> {
   #[track_caller]
   pub fn write_indent(&mut self) -> fmt::Result {
      assert_eq!(self.place, IndentPlace::Start);

      for (count, indent) in &mut self.indents {
         indent(&mut self.inner);
      }

      self.place = IndentPlace::Middle;

      Ok(())
   }
}

impl<W: fmt::Write> fmt::Write for Writer<W> {
   fn write_str(&mut self, s: &str) -> fmt::Result {
      use None as Newline;
      use Some as Line;

      if self.style != style::Style::default() {
         self.inner.write_str(STYLE_RESET)?;
         self.style = style::Style::default();
      }

      let mut lines = Iterator::intersperse(s.split('\n').map(Line), Newline).peekable();
      while let Some(segment) = lines.next() {
         match self.place {
            IndentPlace::Start
               if let Line(line) = segment
                  && !line.is_empty() =>
            {
               self.write_indent()?;
            },

            IndentPlace::End => {
               self.inner.write_char('\n');
               self.place = IndentPlace::Start;
            },

            IndentPlace::Start | IndentPlace::Middle => {},
         }

         match segment {
            Line(line) => {
               self.inner.write_str(line)?;

               if lines.peek().is_none() {
                  self.width = self.width.saturating_add(width(line));
               }
            },

            Newline => {
               self.place = IndentPlace::End;
               self.width = 0;
            },
         }
      }

      Ok(())
   }
}

impl<W: fmt::Write> Write for Writer<W> {
   fn finish(&mut self) -> fmt::Result {
      if self.style != style::Style::default() {
         self.inner.write_str(STYLE_RESET)
      } else {
         Ok(())
      }
   }

   fn width(&self) -> usize {
      self.width
         + if self.place == IndentPlace::Start {
            self.indents.iter().map(|&(count, _)| count as usize).sum()
         } else {
            0
         }
   }

   fn width_max(&self) -> usize {
      self.width_max
   }

   fn dedent<'a>(&'a mut self) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
      let indent = self.indents.pop().expect("dendented without indents");

      guard(self, |this| this.indents.push(indent))
   }

   fn indent_with<'a>(
      &'a mut self,
      count: u8,
      with: IndentWith<Self>,
   ) -> ScopeGuard<&'a mut Self, impl FnOnce(&'a mut Self)> {
      self.indents.push((count, with));

      guard(self, |this| {
         this.indents.pop();
      })
   }

   fn write_style(&mut self, style: style::Style) -> fmt::Result {
      struct Splicer {
         written: bool,
      }

      impl Splicer {
         fn splice(&mut self, writer: &mut impl fmt::Write) -> fmt::Result {
            if self.written {
               writer.write_char(';')
            } else {
               self.written = true;
               writer.write_str("\x1B[")
            }
         }

         fn finish(self, writer: &mut impl fmt::Write) -> fmt::Result {
            if self.written {
               writer.write_char('m')
            } else {
               Ok(())
            }
         }
      }

      let style::Style {
         fg: fg_old,
         bg: bg_old,
         attrs: attrs_old,
      } = self.style;

      let style::Style {
         fg: fg_new,
         bg: bg_new,
         attrs: attrs_new,
      } = style;

      let mut splicer = Splicer { written: false };

      if fg_old != fg_new {
         self.style.fg = fg_new;

         splicer.splice(&mut self.inner)?;
         write_style_color(&mut self.inner, fg_new, StyleColorVariant::Fg)?;
      }

      if bg_old != bg_new {
         self.style.bg = bg_new;

         splicer.splice(&mut self.inner)?;
         write_style_color(&mut self.inner, bg_new, StyleColorVariant::Bg)?;
      }

      let attrs_both = attrs_new & attrs_old;

      for attr_deleted in attrs_old & !attrs_both {
         splicer.splice(&mut self.inner)?;
         self.inner.write_str(style_unattr(attr_deleted))?;
      }

      for attr_added in attrs_new & !attrs_both {
         splicer.splice(&mut self.inner)?;
         self.inner.write_str(style_attr(attr_added))?;
      }

      splicer.finish(&mut self.inner)
   }

   fn write_styled<T: fmt::Display>(&mut self, styled: &style::Styled<T>) -> fmt::Result {
      self.write_style(styled.style)?;
      write!(self.inner, "{value}", value = **styled)
   }

   fn write_report(
      &mut self,
      report: &report::Report,
      location: &impl crate::Display<Self>,
      source: &report::PositionStr<'_>,
   ) -> fmt::Result
   where
      Self: Sized,
   {
      let report::Report {
         severity,
         ref title,
         ref labels,
         ref points,
      } = *report;

      let mut labels: SmallVec<_, 2> = report
         .labels
         .into_iter()
         .map(|label| (source.positions(label.span), label))
         .collect();

      // Sort by line, and when labels are on the same line, sort by column. The one
      // that ends the last will be the last.
      labels.sort_by(|&((a_start, a_end), _), &((b_start, b_end), _)| {
         a_start
            .line
            .cmp(&b_start.line)
            .then_with(|| a_end.column.cmp(&b_end.column))
      });

      let mut lines = SmallVec::<Line, 8>::new();

      'labels: for (label_index, ((label_start, label_end), label)) in
         labels.into_iter().enumerate()
      {
         let label_span_extended = extend_to_line_boundaries(**source, label.span);

         for (line_number, line_content) in
            (label_start.line..).zip(source[label_span_extended.into_std()].split('\n'))
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
                     () if line_is_first => LineStrikeStatus::Start,
                     () if line_is_last => LineStrikeStatus::End,
                     () => LineStrikeStatus::Continue,
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

                  let up_to_start_width = width(&line_content[..*span.start as _]);
                  let label_width = width(&line_content[span.into_std()]);

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
                  let end = source[*start as _..]
                     .find('\n')
                     .map_or(source.size(), |index| start + index);

                  let span = Span::new(start - base, end - base);

                  line.styles.push(LineStyle {
                     span,
                     severity: label.severity,
                  });
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

                  let up_to_end_width = width(&line_content[..*end as _]);

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
               (
                  cmp::Ordering::Equal,
                  report::LabelSeverity::Primary,
                  report::LabelSeverity::Secondary,
               ) => cmp::Ordering::Less,

               (
                  cmp::Ordering::Equal,
                  report::LabelSeverity::Secondary,
                  report::LabelSeverity::Primary,
               ) => cmp::Ordering::Greater,

               (ordering, ..) => ordering,
            }
         });

         line.labels.sort_by_key(|style| {
            // Empty labels are printed offset one column to the right, so treat them like
            // it.
            style.span.end() + u32::from(style.span.is_empty())
         });
      }

      drop(labels);

      let writer = self;

      {
         // INDENT: "<note|warn|error|bug>: "
         let writer = writer.indent_header(
            match severity {
               report::ReportSeverity::Note => "note: ",
               report::ReportSeverity::Warn => "warn: ",
               report::ReportSeverity::Error => "error: ",
               report::ReportSeverity::Bug => "bug: ",
            }
            .style(severity.style_in()),
         );

         wrap(*writer, [title.as_ref().bold()])?;
      }

      let line_number_width = lines.last().map_or(0, |line| number_width(line.number));

      // INDENT: "123 | "
      let line_number = RefCell::new(None::<u32>);
      let line_number_previous = RefCell::new(None::<u32>);
      let writer = writer.indent_with(
         line_number_width as u8 + 3,
         Box::new(move |writer| {
            let line_number = *line_number.borrow();
            borrow_mut!(line_number_previous);

            writer.write_style(STYLE_GUTTER)?;
            match line_number {
               // Don't write the current line number, just print spaces instead.
               None => {
                  write!(writer, "{:>line_number_width$}", "")?;
               },

               // Continuation line. Use dots instead of the number.
               Some(line_number) if *line_number_previous == Some(line_number) => {
                  let dot_width = number_width(line_number);
                  let space_width = line_number_width - dot_width;

                  write!(writer, "{:>space_width$}", "")?;

                  for _ in 0..dot_width {
                     writer.write_char(DOT)?;
                  }
               },

               // New line, but not right after the previous line.
               Some(line_number)
                  if line_number_previous.is_some_and(|line_number_previous| {
                     line_number > line_number_previous + 1
                  }) =>
               {
                  writeln!(
                     writer,
                     "{:>line_number_width$} {TOP_TO_BOTTOM_PARTIAL} ",
                     "",
                  )?;
                  write!(writer, "{line_number:>line_number_width$}")?;
               },

               // New line.
               Some(line_number) => {
                  write!(writer, "{line_number:>line_number_width$}")?;
               },
            }

            write!(writer, " {TOP_TO_BOTTOM} ")?;

            if let Some(line_number) = line_number {
               line_number_previous.replace(line_number);
            }

            Ok(())
         }),
      );

      if let Some(line) = lines.first() {
         {
            // DEDENT: "123 | "
            let writer = writer.dedent();

            // INDENT: "123 ", but spaces.
            let writer = writer.indent(line_number_width as u8 + 1);

            // INDENT: "┏━━━ ".
            let writer = writer.indent_by(
               const_str::concat!(
                  RIGHT_TO_BOTTOM,
                  LEFT_TO_RIGHT,
                  LEFT_TO_RIGHT,
                  LEFT_TO_RIGHT,
                  ' '
               )
               .style(STYLE_GUTTER),
               const_str::concat!(TOP_TO_BOTTOM, "     ").style(STYLE_GUTTER),
            );

            writeln!(writer)?;

            wrap(
               *writer,
               [
                  location
                     .display_free_width()
                     .to_string()
                     .as_str()
                     .style(STYLE_HEADER_PATH),
                  ":".styled(),
                  line
                     .number
                     .to_string()
                     .as_str()
                     .style(STYLE_HEADER_POSITION),
                  ":".styled(),
                  width(&line.content[..*line.styles.first().unwrap().span.start as _])
                     .add(1)
                     .to_string()
                     .as_str()
                     .style(STYLE_HEADER_POSITION),
               ]
               .into_iter(),
            )?;
         }

         writeln!(writer)?;
         writer.write_indent()?;
      }

      let strike_prefix_width = lines
         .iter()
         .map(|line| line.strikes.len())
         .max()
         .unwrap_or(0);

      {
         // INDENT: "<strike-prefix> "
         let strike_prefix = RefCell::new(
            iter::repeat_n(None::<LineStrike>, strike_prefix_width).collect::<SmallVec<_, 2>>(),
         );
         let writer = writer.indent_with(
            strike_prefix_width as u8 + 1,
            Box::new(|writer| {
               const STRIKE_OVERRIDE_DEFAULT: style::Styled<char> = style::Styled::new(' ');

               let mut strike_override = None::<style::Styled<char>>;

               for slot in &*strike_prefix.borrow() {
                  let Some(strike) = *slot else {
                     writer.write_styled(&strike_override.unwrap_or(STRIKE_OVERRIDE_DEFAULT));
                     continue;
                  };

                  match strike.status {
                     LineStrikeStatus::Start => {
                        writer.write_styled(
                           &RIGHT_TO_BOTTOM.style(strike.severity.style_in(severity)),
                        )?;

                        strike_override =
                           Some(LEFT_TO_RIGHT.style(strike.severity.style_in(severity)));
                     },

                     LineStrikeStatus::Continue | LineStrikeStatus::End
                        if let Some(strike) = strike_override =>
                     {
                        writer.write_styled(&strike)?;
                     },

                     LineStrikeStatus::Continue | LineStrikeStatus::End => {
                        writer.write_styled(
                           &TOP_TO_BOTTOM.style(strike.severity.style_in(severity)),
                        )?;
                     },
                  }
               }

               writer.write_styled(&strike_override.unwrap_or(STRIKE_OVERRIDE_DEFAULT))
            }),
         );

         for line in &lines {
            // Patch strike prefix and keep track of positions of strikes with their IDs.
            {
               borrow_mut!(strike_prefix);

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
               line_number.borrow_mut().replace(line.number);

               // Explicitly write the indent because the line may be empty.
               writeln!(writer)?;
               writer.write_indent()?;
               wrap(
                  *writer,
                  resolve_style(&line.content, &line.styles, severity),
               )?;

               *line_number.borrow_mut() = None;
            }

            // Write the line labels.
            // Reverse, because we want to print the labels that end the last first.
            for (label_index, label) in line.labels.iter().enumerate().rev() {
               // HACK: wrap may split the current line into multiple
               // lines, so the label pointer may be too far left.
               // Just max it to 60 for now.
               let span_start = label.span.start().min(Some(60_u32.into()));
               let span_end = label.span.end().min(60_u32.into());

               // DEDENT: "<strike-prefix> "
               let writer = writer.dedent();

               match label.span {
                  LineLabelSpan::UpTo(_) => {
                     let (top_to_right_index, top_to_right) = strike_prefix
                        .borrow()
                        .iter()
                        .enumerate()
                        .rev()
                        .find_map(|(index, strike)| {
                           match strike {
                              &Some(strike) if strike.status == LineStrikeStatus::End => {
                                 Some((index, strike))
                              },

                              _ => None,
                           }
                        })
                        .unwrap();

                     assert_eq!(top_to_right.severity, label.severity);

                     // INDENT: "<strike-prefix>"
                     let mut wrote = false;
                     let writer = writer.indent_with(
                        strike_prefix_width as u8,
                        Box::new(|writer| {
                           // Write all strikes up to the index of the one we are going to
                           // redirect to the right.
                           for slot in strike_prefix.borrow().iter().take(top_to_right_index) {
                              writer.write_styled(&match *slot {
                                 Some(strike) => {
                                    TOP_TO_BOTTOM.style(strike.severity.style_in(severity))
                                 },
                                 None => ' '.styled(),
                              })?;
                           }

                           if wrote {
                              return writer
                                 .write_str(&SPACES[top_to_right_index..strike_prefix_width]);
                           }

                           writer.write_styled(
                              &TOP_TO_RIGHT.style(top_to_right.severity.style_in(severity)),
                           )?;

                           for _ in 0..strike_prefix_width - top_to_right_index - 1 {
                              writer.write_styled(
                                 &LEFT_TO_RIGHT.style(top_to_right.severity.style_in(severity)),
                              )?;
                           }

                           wrote = true;
                           Ok(())
                        }),
                     );

                     // INDENT: "<left-to-right><left-to-bottom>"
                     // INDENT: "               <top--to-bottom>"
                     let mut wrote = false;
                     let writer = writer.indent_with(
                        *span_end as u8 + 2,
                        Box::new(move |writer| {
                           for index in 0..*span_end {
                              writer.write_styled(&match () {
                                 // If there is a label on the current line after this label
                                 // that has a start or
                                 // end at the
                                 // current index, write it instead of out <left-to-right>
                                 () if let Some(label) =
                                    line.labels[..label_index].iter().rev().find(|label| {
                                       *label.span.end() == index && !label.span.is_empty()
                                          || label
                                             .span
                                             .start()
                                             .is_some_and(|start| *start + 1 == index)
                                    }) =>
                                 {
                                    if label.span.is_empty() {
                                       TOP_TO_BOTTOM_LEFT.style(label.severity.style_in(severity))
                                    } else {
                                       TOP_TO_BOTTOM.style(label.severity.style_in(severity))
                                    }
                                 },

                                 () if !wrote => {
                                    LEFT_TO_RIGHT.style(top_to_right.severity.style_in(severity))
                                 },

                                 () => ' '.styled(),
                              })?;
                           }

                           writer.write_styled(
                              &match () {
                                 () if !wrote => LEFT_TO_TOP_BOTTOM,
                                 () => TOP_TO_BOTTOM,
                              }
                              .style(top_to_right.severity.style_in(severity)),
                           )?;

                           writer.write_char(' ')?;

                           wrote = true;
                           strike_prefix.borrow_mut()[top_to_right_index] = None;
                           Ok(())
                        }),
                     );

                     lnwrap(*writer, [label
                        .text
                        .as_ref()
                        .style(top_to_right.severity.style_in(severity))])?;
                  },

                  LineLabelSpan::Inline(_) => {
                     unwrap!(span_start);

                     // INDENT: "<strike-prefix> "
                     let writer = writer.indent_with(
                        strike_prefix_width as u8 + 1,
                        Box::new(|writer| {
                           for slot in &*strike_prefix.borrow() {
                              writer.write_styled(&match *slot {
                                 Some(strike) => {
                                    TOP_TO_BOTTOM.style(strike.severity.style_in(severity))
                                 },
                                 None => ' '.styled(),
                              })?;
                           }

                           writer.write_char(' ')
                        }),
                     );

                     // INDENT: "               <top-to-right><left-to-right><left-to-bottom> "
                     // INDENT: "                                            <top--to-bottom> "
                     // + 1 for extra space.
                     // + 1 if the label is zero-width. The <top-left-to-right> will be placed after
                     //   the span.
                     let mut wrote = false;
                     let writer = writer.indent(Box::new(|writer| {
                        for index in 0..*span_end - u32::from(span_start != span_end) {
                           writer.write_styled(&match () {
                              () if !wrote && index == *span_start => {
                                 TOP_TO_RIGHT.style(label.severity.style_in(severity))
                              },

                              () if let Some(label) =
                                 line.labels[..label_index].iter().rev().find(|label| {
                                    *label.span.end() == index + 1 && !label.span.is_empty()
                                       || label.span.start().is_some_and(|start| *start == index)
                                 }) =>
                              {
                                 if label.span.is_empty() {
                                    TOP_TO_BOTTOM_LEFT.style(label.severity.style_in(severity))
                                 } else {
                                    TOP_TO_BOTTOM.style(label.severity.style_in(severity))
                                 }
                              },

                              () if !wrote && index > *span_start => {
                                 LEFT_TO_RIGHT.style(label.severity.style_in(severity))
                              },

                              () => ' '.styled(),
                           })?;
                        }

                        writer.write_styled(
                           &match *span_end - *span_start {
                              0 if wrote => TOP_TO_BOTTOM_RIGHT,
                              _ if wrote => TOP_TO_BOTTOM,

                              0 => TOP_LEFT_TO_RIGHT,
                              1 => TOP_TO_BOTTOM,

                              _ => LEFT_TO_TOP_BOTTOM,
                           }
                           .style(label.severity.style_in(severity)),
                        )?;
                        writer.write_char(' ')?;

                        wrote = true;
                        Ok(*span_end as usize + usize::from(span_start == span_end) + 1)
                     }));

                     lnwrap(*writer, [label
                        .text
                        .as_ref()
                        .style(label.severity.style_in(severity))])?;
                  },
               }
            }
         }
      }

      // Write the points.
      {
         if !points.is_empty() {
            writeln!(writer)?;
            writer.write_indent()?;
         }

         // DEDENT: "123 | "
         let writer = writer.dedent();

         // INDENT: "123 ", but spaces.
         let writer = writer.indent(line_number_width as u8 + 1);

         for point in points {
            // INDENT: "= "
            let writer = writer.indent_header("= ".style(STYLE_GUTTER));

            // INDENT: "<tip|help|...>: "
            let writer = writer.indent_header(
               match point.severity {
                  report::PointSeverity::Tip => "tip: ",
                  report::PointSeverity::Help => "help: ",
               }
               .style(point.severity.style_in()),
            );

            lnwrap(*writer, [point.text.as_ref().styled()])?;
         }
      }

      Ok(())
   }
}

pub fn writer(inner: impl os::fd::AsFd + fmt::Write) -> impl Write {
   Writer {
      style: style::Style::default(),

      indents: SmallVec::new(),
      place: IndentPlace::Start,

      width: 0,
      width_max: terminal_size::terminal_size_of(&inner.as_fd())
         .map_or(usize::MAX, |(width, _)| width.0 as usize),

      inner,
   }
}

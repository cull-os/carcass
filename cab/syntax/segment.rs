// For the next poor soul that will step in this file:
//
// Beware that changing even the slighest thing will break 500 other cases. Way
// too many hours have been spent on perfecting this, and every single invariant
// is (probably) intended. Please reconsider editing this file.
//
// Comments? Ha!

use std::{
   mem,
   ops,
};

use cab_span::{
   IntoSpan as _,
   Span,
};
use cab_util::{
   Lazy,
   force_ref,
   reffed,
};
use smallvec::SmallVec;
use ust::{
   report::Report,
   style::{
      self,
      StyledExt as _,
   },
};

use crate::{
   node,
   red,
   token,
};

#[must_use]
pub fn unescape(c: char) -> Option<char> {
   Some(match c {
      ' ' => ' ',
      '0' => '\0',
      't' => '\t',
      'n' => '\n',
      'r' => '\r',
      '=' => '=',
      '`' => '`',
      '"' => '\"',
      '\'' => '\'',
      '\\' => '\\',

      _ => return None,
   })
}

pub fn unescape_string(s: &str) -> Result<(String, bool), SmallVec<Span, 4>> {
   let mut string = String::with_capacity(s.len());
   let mut escaped_newline = false;
   let mut invalids = SmallVec::<Span, 4>::new();

   let mut chars = s.char_indices().peekable();
   while let Some((index, c)) = chars.next() {
      if c != '\\' {
         string.push(c);
         continue;
      }

      let Some((_, next)) = chars.next() else {
         // When a string ends with '\', it has to be followed by a newline.
         // And that escapes the newline.
         escaped_newline = true;
         continue;
      };

      let Some(unescaped) = unescape(next) else {
         invalids.push(Span::at(index, '\\'.len_utf8() + next.len_utf8()));
         continue;
      };

      string.push(unescaped);
   }

   if invalids.is_empty() {
      Ok((string, escaped_newline))
   } else {
      Err(invalids)
   }
}

#[bon::builder]
pub fn escape(
   #[builder(start_fn)] c: char,
   delimiter: Option<(char, &'static str)>,
) -> Option<&'static str> {
   Some(match c {
      '\0' => "\\0",
      '\t' => "\\t",
      '\n' => "\\n",
      '\r' => "\\r",

      c if let Some((delimiter, delimiter_escaped)) = delimiter
         && c == delimiter =>
      {
         delimiter_escaped
      },

      _ => return None,
   })
}

#[bon::builder]
pub fn escape_string<'a>(
   #[builder(start_fn)] s: &'a str,
   #[builder(default)] normal_style: style::Style,
   #[builder(default)] escaped_style: style::Style,
   delimiter: Option<(char, &'static str)>,
) -> impl Iterator<Item = style::Styled<&'a str>> {
   // Bon doesn't like generator syntax.
   escape_string_impl(s, normal_style, escaped_style, delimiter)
}

fn escape_string_impl<'a>(
   s: &'a str,
   normal: style::Style,
   escaped: style::Style,
   delimiter: Option<(char, &'static str)>,
) -> impl Iterator<Item = style::Styled<&'a str>> {
   gen move {
      let mut literal_start_offset = 0;

      for (offset, c) in s.char_indices() {
         let Some(escaped_) = escape(c).maybe_delimiter(delimiter).call() else {
            continue;
         };

         yield s[literal_start_offset..offset].style(normal);
         literal_start_offset = offset;

         yield escaped_.style(escaped);
         literal_start_offset += c.len_utf8();
      }

      yield s[literal_start_offset..s.len()].style(normal);
   }
}

reffed! {
   #[derive(Debug, Clone, PartialEq, Eq, Hash)]
   enum SegmentRaw {
      Content(token::Content),
      Interpolation(node::Interpolation),
   }
}

impl SegmentRawRef<'_> {
   #[must_use]
   fn span(self) -> Span {
      match self {
         Self::Content(content) => content.span(),
         Self::Interpolation(interpolation) => interpolation.span(),
      }
   }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment<'a> {
   Content { span: Span, content: String },
   Interpolation(&'a node::Interpolation),
}

impl Segment<'_> {
   #[must_use]
   pub fn is_content(&self) -> bool {
      matches!(self, &Self::Content { .. })
   }

   #[must_use]
   pub fn is_interpolation(&self) -> bool {
      matches!(self, &Self::Interpolation(_))
   }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Straight<'a> {
   Line {
      span: Span,
      text: &'a str,

      is_from_line_start: bool,
      is_to_line_end:     bool,

      is_first: bool,
      is_last:  bool,
   },

   Interpolation(&'a node::Interpolation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segments<'a> {
   span: Span,

   pub is_multiline: bool,

   line_span_first: Option<Span>,
   line_span_last:  Option<Span>,

   straights: SmallVec<Straight<'a>, 4>,
}

impl<'a> IntoIterator for Segments<'a> {
   type Item = Segment<'a>;

   type IntoIter = impl Iterator<Item = Segment<'a>>;

   fn into_iter(self) -> Self::IntoIter {
      gen move {
         let mut buffer = String::new();
         let mut buffer_span = None::<Span>;

         let (indent, indent_width) = self
            .indent()
            .expect("string must be valid and not mix indents");

         for straight in self.straights {
            match straight {
               Straight::Line {
                  span,
                  mut text,
                  is_from_line_start,
                  is_to_line_end,
                  is_first,
                  is_last,
               } => {
                  if self.is_multiline {
                     // Multiline strings' first and last lines are ignored:
                     //
                     // "<ignored>
                     // <content>
                     // <ignored>"
                     if is_first || is_last {
                        assert!(
                           text.chars().all(char::is_whitespace),
                           "multiline string must be valid and not have non-whitespace characters \
                            in first and last lines"
                        );
                        continue;
                     }

                     if is_to_line_end {
                        text = text.trim_end();
                     }

                     if is_from_line_start {
                        text = if text.chars().all(char::is_whitespace) {
                           ""
                        } else {
                           assert!(
                              text[..indent_width].chars().all(|c| c == indent.unwrap()),
                              "multiline string must be valid and not mix indents"
                           );
                           &text[indent_width..]
                        }
                     }
                  }

                  let (unescaped, escaped_newline) =
                     unescape_string(text).expect("string content must be valid");

                  buffer.push_str(&unescaped);

                  if is_to_line_end && !escaped_newline {
                     buffer.push('\n');
                  }

                  buffer_span.replace(buffer_span.map_or(span, |span_| span_.cover(span)));
               },

               Straight::Interpolation(interpolation) => {
                  yield Segment::Content {
                     span:    buffer_span
                        .take()
                        .expect("interpolation must never be the first or last segment"),
                     content: mem::take(&mut buffer),
                  };

                  yield Segment::Interpolation(interpolation);
               },
            }
         }

         if let Some(span) = buffer_span {
            yield Segment::Content {
               span,
               content: buffer,
            };
         }
      }
   }
}

impl Segments<'_> {
   fn indent(&self) -> Result<(Option<char>, usize), SmallVec<char, 4>> {
      let mut indents = SmallVec::<char, 4>::new();
      let mut indent_width = None::<usize>;

      for straight in &self.straights {
         let &Straight::Line {
            text,
            is_from_line_start: true,
            is_last: false,
            ..
         } = straight
         else {
            continue;
         };

         if text.chars().all(char::is_whitespace) {
            continue;
         }

         let mut line_indent_width: usize = 0;

         for c in text.chars() {
            if !c.is_whitespace() {
               break;
            }

            line_indent_width += 1;

            if !indents.contains(&c) {
               indents.push(c);
            }
         }

         if let Some(width) = indent_width {
            indent_width.replace(width.min(line_indent_width));
         } else {
            indent_width.replace(line_indent_width);
         }
      }

      if indents.len() > 1 {
         return Err(indents);
      }

      Ok((indents.first().copied(), indent_width.unwrap_or(0)))
   }

   pub fn validate(&self, to: &mut Vec<Report>, report: &mut Lazy!(Report)) {
      for straight in &self.straights {
         match *straight {
            Straight::Line { span, text, .. } => {
               if let Err(invalids) = unescape_string(text) {
                  for invalid in invalids {
                     force_ref!(report).push_primary(invalid.offset(span.start), "invalid escape");
                  }
               }
            },

            Straight::Interpolation(interpolation) => interpolation.expression().validate(to),
         }
      }

      if let Err(indents) = self.indent() {
         force_ref!(report).push_primary(
            self.span,
            format!(
               "cannot mix different kinds of space in indents: {indents}",
               indents = indents
                  .into_iter()
                  .map(|c| {
                     match escape(c).delimiter(('\'', "\\'")).call() {
                        Some(escaped) => escaped.to_owned(),
                        None => format!("'{c}'"),
                     }
                  })
                  .intersperse(", ".to_owned())
                  .collect::<String>(),
            ),
         );
      }

      if self.is_multiline {
         for span in [self.line_span_first, self.line_span_last]
            .into_iter()
            .flatten()
         {
            force_ref!(report).push_primary(span, "first and last lines must be empty");
         }
      }
   }
}

pub trait Segmented: ops::Deref<Target = red::Node> {
   fn segments(&self) -> Segments<'_> {
      let mut is_multiline = false;

      let mut line_span_first = None::<Span>;
      let mut line_span_last = None::<Span>;

      let mut straights = SmallVec::new();

      let mut previous_segment_span = None::<Span>;
      let mut segments = self
         .children_with_tokens()
         .filter_map(|child| {
            match child {
               red::ElementRef::Node(node) => {
                  Some(SegmentRawRef::Interpolation(
                     <&node::Interpolation>::try_from(node)
                        .expect("child node of segmented node must be interpolation"),
                  ))
               },

               // The reason we are not asserting here is because invalid
               // segmented nodes sometimes contain non-content tokens,
               // it's not worth it to fix this as it'll error anyway.
               red::ElementRef::Token(token) => {
                  <&token::Content>::try_from(token)
                     .map(SegmentRawRef::Content)
                     .ok()
               },
            }
         })
         .enumerate()
         .peekable();

      while let Some((segment_index, segment)) = segments.next() {
         let mut segment_is_multiline = false;

         let segment_is_first = segment_index == 0;
         let segment_is_last = segments.peek().is_none();

         match segment {
            SegmentRawRef::Content(content) => {
               let span = content.span();

               let mut offset: usize = 0;
               let mut lines = content.text().split('\n').enumerate().peekable();
               while let Some((line_index, line)) = lines.next() {
                  let line_is_first = line_index == 0;
                  let line_is_last = lines.peek().is_none();

                  if line_is_first && !line_is_last {
                     segment_is_multiline = true;
                  }

                  if segment_is_first && line_is_first {
                     let suffix_interpolation_span = line_is_last
                        .then(|| segments.peek().map(|&(_, segment)| segment.span()))
                        .flatten();

                     if let Some(interpolation_span) = suffix_interpolation_span {
                        line_span_first.replace(span.cover(interpolation_span));
                     } else {
                        let line = line.trim_end();

                        if !line.is_empty() {
                           line_span_first.replace(Span::at(span.start, line.len()));
                        }
                     }
                  }

                  if segment_is_last && line_is_last {
                     let prefix_interpolation_span =
                        line_is_first.then_some(previous_segment_span).flatten();

                     if let Some(interpolation_span) = prefix_interpolation_span {
                        line_span_last.replace(span.cover(interpolation_span));
                     } else {
                        let line = line.trim_start();

                        if !line.is_empty() {
                           line_span_last.replace(Span::at_end(span.end, line.len()));
                        }
                     }
                  }

                  #[expect(clippy::nonminimal_bool)]
                  straights.push(Straight::Line {
                     span: Span::at(content.span().start + offset, line.len()),

                     text: &content.text()[offset..offset + line.len()],

                     is_from_line_start: !(segment_is_first && line_is_first)
                        && !(previous_segment_span.is_some() && line_is_first),
                     is_to_line_end:     !line_is_last,

                     is_first: segment_is_first && line_is_first,
                     is_last:  segment_is_last && line_is_last,
                  });

                  offset += line.len() + '\n'.len_utf8();
               }
            },

            SegmentRawRef::Interpolation(interpolation) => {
               let span = interpolation.span();

               if segment_is_first {
                  line_span_first.replace(span);
               }

               if segment_is_last {
                  line_span_last.replace(span);
               }

               straights.push(Straight::Interpolation(interpolation));
            },
         }

         previous_segment_span.replace(segment.span());

         if segment_is_multiline {
            is_multiline = true;
         }
      }

      Segments {
         span: self.span(),

         is_multiline,

         line_span_first,
         line_span_last,

         straights,
      }
   }

   fn is_trivial(&self) -> bool {
      let mut segments = self.segments().into_iter().peekable();

      segments.next().is_some_and(|segment| segment.is_content()) && segments.peek().is_none()
   }
}

use std::ops;

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

#[must_use]
pub fn unescape_string(s: &str) -> Option<String> {
   let mut string = String::with_capacity(s.len());

   let mut literal_start_offset = 0;

   let mut chars = s.char_indices();
   while let Some((offset, c)) = chars.next() {
      if c != '\\' {
         continue;
      }

      string.push_str(&s[literal_start_offset..offset]);
      literal_start_offset = offset;

      let (_, c) = chars.next()?;
      string.push(unescape(c)?);
      literal_start_offset += '\\'.len_utf8() + c.len_utf8();
   }

   string.push_str(&s[literal_start_offset..s.len()]);
   Some(string)
}

pub fn escape(c: char) -> Option<&'static str> {
   Some(match c {
      '\0' => "\\0",
      '\t' => "\\t",
      '\n' => "\\n",
      '\r' => "\\r",

      _ => return None,
   })
}

pub fn escape_string(s: &str, normal: style::Style) -> impl Iterator<Item = style::Styled<&str>> {
   gen move {
      let mut literal_start_offset = 0;

      for (offset, c) in s.char_indices() {
         let Some(escaped) = escape(c) else {
            continue;
         };

         yield s[literal_start_offset..offset].style(normal);
         literal_start_offset = offset;

         yield escaped.magenta().bold();
         literal_start_offset += c.len_utf8();
      }

      yield s[literal_start_offset..s.len()].style(normal);
   }
}

type Indent = (Option<char>, usize);

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

#[derive(Debug)]
enum Straight<'a> {
   Line {
      span:               Span,
      text:               &'a str,
      is_from_line_start: bool,

      is_first: bool,
      is_last:  bool,
   },

   Interpolation(&'a node::Interpolation),
}

#[derive(Debug)]
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

         let (indent, indent_width) = self.calculate_indent();

         for straight in self.straights {
            match straight {
               Straight::Line {
                  span,
                  text,
                  is_from_line_start,
                  is_first,
                  is_last,
               } => {
                  let unindented = if is_last && is_from_line_start {
                     text.trim_start()
                  } else if is_from_line_start {
                     assert!(text[..indent_width].chars().all(|c| c == indent.unwrap()));
                     &text[indent_width..]
                  } else {
                     text
                  };

                  buffer.push_str(&unescape_string(unindented).unwrap());

                  if !is_first && !is_last {
                     buffer.push('\n');
                  }

                  buffer_span.replace(buffer_span.map_or(span, |span_| span_.cover(span)));
               },

               Straight::Interpolation(interpolation) => {
                  yield Segment::Content {
                     span:    buffer_span.take().unwrap(),
                     content: buffer.clone(),
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
   fn calculate_indent(&self) -> Indent {
      let mut indent = None::<char>;
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

         let mut line_indent_width: usize = 0;

         for c in text.chars() {
            if !c.is_whitespace() {
               break;
            }

            line_indent_width += 1;

            match indent {
               None => indent = Some(c),
               Some(indent) => assert_eq!(indent, c),
            }
         }

         if let Some(width) = indent_width {
            indent_width.replace(width.min(line_indent_width));
         } else {
            indent_width.replace(line_indent_width);
         }
      }

      (indent, indent_width.unwrap_or(0))
   }

   pub fn validate(&self, report: &mut Lazy!(Report), to: &mut Vec<Report>) {
      for straight in &self.straights {
         match *straight {
            Straight::Line { span, text, .. } => {
               let mut chars = text.char_indices();
               while let Some((offset, c)) = chars.next() {
                  if c != '\\' {
                     continue;
                  }

                  match chars.next() {
                     Some((_, c)) if unescape(c).is_some() => {},

                     next @ (Some(_) | None) => {
                        force_ref!(report).push_primary(
                           Span::at(
                              span.start + offset,
                              1 + next.map_or(0, |(_, c)| c.len_utf8()),
                           ),
                           "invalid escape",
                        );
                     },
                  }
               }
            },

            Straight::Interpolation(interpolation) => interpolation.expression().validate(to),
         }

         let Straight::Line {
            text,
            is_from_line_start: true,
            ..
         } = *straight
         else {
            continue;
         };

         let mut indent = None::<char>;
         for c in text.chars() {
            if !c.is_whitespace() {
               break;
            }

            let Some(indent) = indent else {
               indent.replace(c);
               continue;
            };

            if indent != c {
               force_ref!(report)
                  .push_primary(self.span, "cannot mix different kinds of space in indents");
               break;
            }
         }
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
                     node
                        .try_into()
                        .expect("child node of segmented node must be interpolation"),
                  ))
               },

               red::ElementRef::Token(token) => token.try_into().map(SegmentRawRef::Content).ok(),
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
                     if !line.trim().is_empty() {
                        line_span_first.replace(Span::at(span.start, line.trim_end().len()));
                     } else if let Some(&(_, segment)) = segments.peek() {
                        line_span_first.replace(span.cover(segment.span()));
                     }
                  }

                  if segment_is_last && line_is_last {
                     if !line.trim().is_empty() {
                        line_span_last.replace(Span::at_end(span.end, line.trim_start().len()));
                     } else if let Some(previous_span) = previous_segment_span {
                        line_span_last.replace(span.cover(previous_span));
                     }
                  }

                  #[expect(clippy::nonminimal_bool)]
                  straights.push(Straight::Line {
                     span: Span::at(content.span().start + offset, line.len()),

                     text: &content.text()[offset..offset + line.len()],

                     is_from_line_start: !(segment_is_first && line_is_first)
                        && !(previous_segment_span.is_some() && line_is_first),

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
}

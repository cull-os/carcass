use std::{
   borrow::Cow,
   fmt::{
      self,
      Write as _,
   },
   mem,
   slice,
   sync::RwLock,
};

use derive_more::Deref;

use crate::{
   Debug,
   Display,
   Write,
   report,
   style::{
      self,
      StyledExt as _,
   },
   write,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag<'a> {
   Text(style::Styled<Cow<'a, str>>),
   Space,
   Newline(usize),
   Group(usize),
   Indent(isize),
}

impl<'a, I: Into<Cow<'a, str>>> From<I> for Tag<'a> {
   fn from(value: I) -> Self {
      Self::Text(value.into().styled())
   }
}

impl<'a, I: Into<Cow<'a, str>>> From<style::Styled<I>> for Tag<'a> {
   fn from(value: style::Styled<I>) -> Self {
      Self::Text(value.value.into().style(value.style))
   }
}

impl Tag<'_> {
   #[must_use]
   pub fn is_node(&self) -> bool {
      matches!(*self, Self::Group(..) | Self::Indent(..))
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
   Flat,
   Broken,
   Always,
}

#[derive(Debug, Clone, Copy)]
struct Measure {
   width:  usize,
   column: usize,
}

#[derive(Debug)]
struct Data<'a> {
   tag:       Tag<'a>,
   len:       usize,
   condition: Condition,
   measure:   RwLock<Measure>,
}

impl Data<'_> {
   fn measure(&self, children: TagsIter<'_>) {
      let tag_width = match self.tag {
         Tag::Indent(..) if self.condition == Condition::Broken => 0,
         _ if self.condition == Condition::Broken => 0,

         Tag::Text(ref s) if s.contains('\n') => usize::MAX,
         Tag::Text(ref s) => super::width(s),

         Tag::Space => 1,
         Tag::Newline(_) => usize::MAX,

         Tag::Group(..) | Tag::Indent(..) => 0,
      };

      let mut measure = self.measure.write().unwrap();

      *measure = Measure {
         width: children
            .map(|(child, children)| {
               child.measure(children);
               measure.width
            })
            .fold(tag_width, usize::saturating_add),

         column: 0,
      };
   }
}

#[derive(Debug)]
pub struct Tags<'a>(Vec<Data<'a>>);

pub trait DisplayTags {
   fn display_tags<'a>(&'a self, tags: &mut Tags<'a>);
}

impl<D: DisplayTags> Display for D {
   fn display_styled(&self, writer: &mut dyn Write) -> fmt::Result {
      let tags: Tags<'_> = self.into();
      tags.display_styled(writer)
   }
}

impl<'a, D: DisplayTags> From<&'a D> for Tags<'a> {
   fn from(value: &'a D) -> Self {
      let mut this = Self(Vec::new());
      value.display_tags(&mut this);
      this
   }
}

impl Debug for Tags<'_> {
   fn debug_styled(&self, writer: &mut dyn Write) -> fmt::Result {
      fn debug(writer: &mut dyn Write, children: TagsIter<'_>) -> fmt::Result {
         for (index, (data, children)) in children.enumerate() {
            if index != 0 {
               writeln!(writer)?;
            }

            let condition = match data.condition {
               Condition::Flat => " if=flat",
               Condition::Broken => " if=broken",
               Condition::Always => "",
            };

            match data.tag {
               Tag::Text(ref s) => {
                  write!(writer, "<text{condition}>")?;
                  write(writer, s)?;
                  write!(writer, "</text>")?;
               },
               Tag::Space => write!(writer, "<space{condition}/>")?,
               Tag::Newline(count) => write!(writer, "<newline count={count}{condition}>")?,

               ref tag @ (Tag::Group(..) | Tag::Indent(..)) => {
                  let text = match *tag {
                     Tag::Group(..) => "group",
                     Tag::Indent(..) => "indent",
                     _ => unreachable!(),
                  };

                  if children.len() == 0 {
                     write!(writer, "<{text}{condition}/>")?;
                     continue;
                  }

                  writeln!(writer, "<{text}{condition}>")?;
                  {
                     super::indent!(writer, 3);
                     debug(writer, children)?;
                     writeln!(writer)?;
                  }
                  write!(writer, "</{text}>")?;
               },
            }
         }

         Ok(())
      }

      debug(writer, self.children())
   }
}

impl Display for Tags<'_> {
   fn display_styled(&self, writer: &mut dyn Write) -> fmt::Result {
      struct Renderer<'a> {
         inner:    &'a mut dyn Write,
         indent:   usize,
         space:    bool,
         newlines: usize,
      }

      impl Write for Renderer<'_> {
         fn finish(&mut self) -> fmt::Result {
            self.inner.finish()
         }

         fn width(&self) -> usize {
            self.inner.width()
         }

         fn width_max(&self) -> usize {
            self.inner.width_max()
         }

         fn get_style(&self) -> style::Style {
            self.inner.get_style()
         }

         fn set_style(&mut self, style: style::Style) {
            self.inner.set_style(style);
         }

         fn apply_style(&mut self) -> fmt::Result {
            self.inner.apply_style()
         }

         fn write_report(
            &mut self,
            report: &report::Report,
            location: &dyn crate::Display,
            source: &report::PositionStr<'_>,
         ) -> fmt::Result {
            self.inner.write_report(report, location, source)
         }
      }

      impl fmt::Write for Renderer<'_> {
         fn write_str(&mut self, s: &str) -> fmt::Result {
            if s.is_empty() {
               return Ok(());
            }

            if mem::take(&mut self.space) && !s.starts_with('\n') {
               self.write_char(' ')?;
            }

            for line in s.split_inclusive('\n') {
               if line == "\n" {
                  self.newlines += 1;
                  self.inner.write_char('\n')?;
                  continue;
               }

               if mem::take(&mut self.newlines) > 0 {
                  for _ in 0..self.indent {
                     self.inner.write_char(' ')?;
                  }
               }

               self.inner.write_str(line)?;

               if line.ends_with('\n') {
                  self.newlines = 1;
               }
            }

            Ok(())
         }
      }

      impl Renderer<'_> {
         fn render(&mut self, children: TagsIter<'_>, parent_is_broken: bool) -> fmt::Result {
            for (data, children) in children {
               let condition = match data.condition {
                  Condition::Flat => !parent_is_broken,
                  Condition::Broken => parent_is_broken,
                  Condition::Always => true,
               };

               match data.tag {
                  Tag::Text(ref s) => {
                     if condition {
                        write(self, s)?;
                     }
                  },

                  Tag::Space => {
                     if condition {
                        self.space = true;
                     }
                  },

                  Tag::Newline(count) => {
                     if condition {
                        for _ in self.newlines..count {
                           self.write_char('\n')?;
                        }
                     }
                  },

                  Tag::Group(..) => {
                     let measure = data.measure.read().unwrap();
                     self.render(children, measure.width == usize::MAX)?;
                  },

                  Tag::Indent(count) => {
                     if condition {
                        self.indent = self.indent.checked_add_signed(count).unwrap();
                        self.render(children, parent_is_broken)?;
                        self.indent = self.indent.checked_sub_signed(count).unwrap();
                     } else {
                        self.render(children, parent_is_broken)?;
                     }
                  },
               }
            }

            Ok(())
         }
      }

      self.layout(writer.width_max().saturating_sub(writer.width()));

      Renderer {
         inner:    writer,
         indent:   0,
         space:    false,
         newlines: 0,
      }
      .render(self.children(), false)
   }
}

impl<'a> Tags<'a> {
   fn layout(&self, column_max: usize) {
      #[derive(Debug)]
      struct Layer {
         indent: usize,

         column:     usize,
         column_max: usize,
      }

      impl Layer {
         fn layout(&mut self, children: TagsIter<'_>) {
            for (data, children) in children {
               let mut measure = data.measure.write().unwrap();
               measure.column = self.column;

               let condition = data.condition != Condition::Flat;

               match data.tag {
                  Tag::Text(ref s) if let Some(nl) = s.rfind('\n') => {
                     self.column = self.indent + super::width(&s[nl..]);
                  },

                  Tag::Text(_) => self.column += measure.width,

                  Tag::Space => self.column += 1,

                  Tag::Newline(0) => {},
                  Tag::Newline(_) => self.column = self.indent,

                  Tag::Group(max) => {
                     let width = match self.column.saturating_add(measure.width) {
                        width if width > self.column_max => usize::MAX,
                        width if width > max => usize::MAX,
                        width => width,
                     };

                     if width < usize::MAX {
                        self.column += width;
                     } else {
                        measure.width = usize::MAX;
                        self.layout(children);
                     }
                  },

                  Tag::Indent(count) => {
                     if condition {
                        self.indent = self.indent.checked_add_signed(count).unwrap();
                     }

                     self.layout(children);

                     if condition {
                        self.indent = self.indent.checked_sub_signed(count).unwrap();
                     }
                  },
               }
            }
         }
      }

      for (data, children) in self.children() {
         data.measure(children);
      }

      Layer {
         indent: 0,
         column: 0,
         column_max,
      }
      .layout(self.children());
   }

   pub fn write(&mut self, tag: impl Into<Tag<'a>>) {
      self.write_if(tag, Condition::Always);
   }

   pub fn write_if(&mut self, tag: impl Into<Tag<'a>>, condition: Condition) {
      self.write_if_with(tag, condition, |_| {});
   }

   pub fn write_with(&mut self, tag: impl Into<Tag<'a>>, closure: impl FnOnce(&mut Self)) {
      self.write_if_with(tag, Condition::Always, closure);
   }

   pub fn write_if_with(
      &mut self,
      tag: impl Into<Tag<'a>>,
      condition: Condition,
      closure: impl FnOnce(&mut Self),
   ) {
      let tag = tag.into();

      let tag_is_node = tag.is_node();
      let tag_should_pop =
         tag == Tag::Space && self.0.last().is_some_and(|data| data.tag == Tag::Space);

      let index = self.0.len();
      self.0.push(Data {
         tag,
         len: 0,
         condition,
         measure: RwLock::new(Measure {
            width:  0,
            column: 0,
         }),
      });

      let len = self.0.len();
      closure(self);
      let len = self.0.len() - len;

      assert!(
         tag_is_node || len == 0,
         "inserted children for non-node {tag:?}",
         tag = self.0[index].tag
      );

      if tag_should_pop {
         self.0.pop();
      } else {
         self.0[index].len = len;
      }
   }

   fn children(&self) -> TagsIter<'_> {
      TagsIter(self.0.iter())
   }
}

#[derive(Deref)]
struct TagsIter<'a>(slice::Iter<'a, Data<'a>>);

impl<'a> Iterator for TagsIter<'a> {
   type Item = (&'a Data<'a>, TagsIter<'a>);

   fn next(&mut self) -> Option<Self::Item> {
      let this = self.0.next()?;

      if this.len == 0 {
         return Some((this, TagsIter([].iter())));
      }

      let (children, rest) = self.0.as_slice().split_at(this.len);

      self.0 = rest.iter();

      Some((this, TagsIter(children.iter())))
   }
}

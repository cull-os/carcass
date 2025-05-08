use std::{
   borrow::Cow,
   cell::Cell,
   fmt::{
      self,
      Write as _,
   },
   mem,
   slice,
};

use cab_util::into;
use derive_more::Deref;

use crate::{
   DebugView,
   DisplayView,
   WriteView,
   indent,
   width,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag<'a> {
   Text(Cow<'a, str>),
   Space,
   Newline(usize),
   Group(usize),
   Indent(isize),
}

impl<'a, I: Into<Cow<'a, str>>> From<I> for Tag<'a> {
   fn from(value: I) -> Self {
      into!(value);
      Self::Text(value)
   }
}

impl Tag<'_> {
   #[must_use]
   pub fn is_node(&self) -> bool {
      matches!(*self, Self::Group(..) | Self::Indent(..))
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagCondition {
   Flat,
   Broken,
   Always,
}

#[derive(Debug, Clone, Copy)]
struct Measure {
   width:  usize,
   column: usize,
}

#[derive(Debug, Clone)]
struct TagData<'a> {
   tag:       Tag<'a>,
   len:       usize,
   condition: TagCondition,
   measure:   Cell<Measure>,
}

impl TagData<'_> {
   fn measure(&self, children: TagsIter<'_>) {
      let tag_width = match self.tag {
         Tag::Indent(..) if self.condition == TagCondition::Broken => 0,
         _ if self.condition == TagCondition::Broken => 0,

         Tag::Text(ref s) if s.contains('\n') => usize::MAX,
         Tag::Text(ref s) => width(s),

         Tag::Space => 1,
         Tag::Newline(_) => usize::MAX,

         Tag::Group(..) | Tag::Indent(..) => 0,
      };

      let width = children
         .map(|(child, children)| {
            child.measure(children);
            child.measure.get().width
         })
         .fold(tag_width, usize::saturating_add);

      self.measure.set(Measure { width, column: 0 });
   }
}

#[derive(Clone)]
pub struct Tags<'a>(Vec<TagData<'a>>);

impl DebugView for Tags<'_> {
   fn debug(&self, writer: &mut dyn WriteView) -> fmt::Result {
      fn debug(writer: &mut dyn WriteView, children: TagsIter<'_>) -> fmt::Result {
         for (index, (data, children)) in children.enumerate() {
            if index != 0 {
               writeln!(writer)?;
            }

            let condition = match data.condition {
               TagCondition::Flat => " if=flat",
               TagCondition::Broken => " if=broken",
               TagCondition::Always => "",
            };

            match data.tag {
               Tag::Text(ref s) => write!(writer, "<text{condition}>{s:?}</text>")?,
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
                     indent!(writer, 3);
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

impl DisplayView for Tags<'_> {
   fn display(&self, writer: &mut dyn WriteView) -> fmt::Result {
      struct Renderer<'a> {
         writer:   &'a mut dyn WriteView,
         indent:   usize,
         space:    bool,
         newlines: usize,
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
                  self.writer.write_char('\n')?;
                  continue;
               }

               if mem::take(&mut self.newlines) > 0 {
                  for _ in 0..self.indent {
                     self.writer.write_char(' ')?;
                  }
               }

               self.writer.write_str(line)?;

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
                  TagCondition::Flat => !parent_is_broken,
                  TagCondition::Broken => parent_is_broken,
                  TagCondition::Always => true,
               };

               match data.tag {
                  Tag::Text(ref s) => {
                     if condition {
                        self.write_str(s)?;
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
                     let measure = data.measure.get();
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
         writer,
         indent: 0,
         space: false,
         newlines: 0,
      }
      .render(self.children(), false)
   }
}

#[derive(Deref)]
struct TagsIter<'a>(slice::Iter<'a, TagData<'a>>);

impl<'a> Iterator for TagsIter<'a> {
   type Item = (&'a TagData<'a>, TagsIter<'a>);

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

impl<'a> Tags<'a> {
   #[must_use]
   pub fn new() -> Self {
      Self(Vec::new())
   }

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
               let mut measure = data.measure.get();
               measure.column = self.column;

               let condition = data.condition != TagCondition::Flat;

               match data.tag {
                  Tag::Text(ref s) if let Some(nl) = s.rfind('\n') => {
                     self.column = self.indent + width(&s[nl..]);
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

               data.measure.set(measure);
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
      self.write_if(tag, TagCondition::Always);
   }

   pub fn write_if(&mut self, tag: impl Into<Tag<'a>>, condition: TagCondition) {
      self.write_if_with(tag, condition, |_| {});
   }

   pub fn write_with(&mut self, tag: impl Into<Tag<'a>>, closure: impl FnOnce(&mut Self)) {
      self.write_if_with(tag, TagCondition::Always, closure);
   }

   pub fn write_if_with(
      &mut self,
      tag: impl Into<Tag<'a>>,
      condition: TagCondition,
      closure: impl FnOnce(&mut Self),
   ) {
      into!(tag);
      let tag_is_node = tag.is_node();
      let tag_should_pop =
         tag == Tag::Space && self.0.last().is_some_and(|data| data.tag == Tag::Space);

      let index = self.0.len();
      self.0.push(TagData {
         tag,
         len: 0,
         condition,
         measure: Cell::new(Measure {
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

use std::{
   borrow::Cow,
   cell::Cell,
   fmt,
   slice,
};

use cab_util::into;
use derive_more::{
   Deref,
   DerefMut,
};

use crate::{
   DebugView,
   DisplayView,
   WriteView,
   indent,
};

const INDENT_SIZE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tag<'a> {
   Text(Cow<'a, str>),
   Space,
   Newline(usize),
   Group,
   Indent,
}

impl Tag<'_> {
   #[must_use]
   pub fn is_node(&self) -> bool {
      matches!(*self, Self::Group | Self::Indent)
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
pub struct TagData<'a> {
   tag:       Tag<'a>,
   len:       usize,
   condition: TagCondition,
   measure:   Cell<Measure>,
}

#[derive(Deref, DerefMut, Clone)]
pub struct Tags<'a>(Vec<TagData<'a>>);

impl DebugView for Tags<'_> {
   fn debug(&self, writer: &mut dyn WriteView) -> fmt::Result {
      for (data, children) in self.iter() {
         match data.tag {
            Tag::Text(ref s) => write!(writer, "<text>{s:?}</text>")?,
            Tag::Space => write!(writer, "<space/>")?,
            Tag::Newline(count) => write!(writer, "<newline count={count}>")?,

            Tag::Group => {
               if children.len() == 0 {
                  write!(writer, "<group/>")?;
                  continue;
               }

               write!(writer, "<group>")?;
               {
                  indent!(writer, INDENT_SIZE);
                  data.debug(writer)?;
               }
               write!(writer, "</group>")?;
            },

            Tag::Indent => todo!(),
         }
      }

      Ok(())
   }
}

impl<'a> Tags<'a> {
   #[must_use]
   pub fn new() -> Self {
      Self(Vec::new())
   }

   #[must_use]
   pub fn render(&self) -> impl DisplayView + '_ {
      #[derive(Deref, DerefMut)]
      struct Tags_<'a>(&'a Tags<'a>);

      impl DisplayView for Tags_<'_> {
         fn display(&self, writer: &mut dyn WriteView) -> fmt::Result {
            todo!()
         }
      }

      Tags_(self)
   }

   pub fn write(&mut self, tag: impl Into<Tag<'a>>) {
      self.write_if(tag, TagCondition::Always);
   }

   pub fn write_if(&mut self, tag: impl Into<Tag<'a>>, condition: TagCondition) {
      self.write_with_if(tag, |_| {}, condition);
   }

   pub fn write_with(&mut self, tag: impl Into<Tag<'a>>, closure: impl FnOnce(&mut Self)) {
      self.write_with_if(tag, closure, TagCondition::Always);
   }

   pub fn write_with_if(
      &mut self,
      tag: impl Into<Tag<'a>>,
      closure: impl FnOnce(&mut Self),
      condition: TagCondition,
   ) {
      into!(tag);
      let tag_is_node = tag.is_node();
      let tag_should_pop =
         tag == Tag::Space && self.last().is_some_and(|data| data.tag == Tag::Space);

      let index = self.len();
      self.push(TagData {
         tag,
         len: 0,
         condition,
         measure: Cell::new(Measure {
            width:  0,
            column: 0,
         }),
      });

      let len = self.len();
      closure(self);
      let len = len - self.len();

      assert!(
         tag_is_node || len == 0,
         "inserted children for non-node {tag:?}",
         tag = self[index].tag
      );

      if tag_should_pop {
         self.pop();
      } else {
         self[index].len = len;
      }
   }

   fn iter(&'a self) -> impl Iterator<Item = (&'a TagData<'a>, slice::Iter<'a, TagData<'a>>)> {
      gen {
         let mut content = (**self).iter();

         while let Some(data) = content.next() {
            if data.len == 0 {
               yield (data, [].iter());
               continue;
            }

            let (this, rest) = content.as_slice().split_at(data.len);
            content = rest.iter();

            yield (data, this.iter());
         }
      }
   }
}

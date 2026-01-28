use std::sync::Arc;

use cab_util::into;
use dup::Dupe;
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use super::Value;
use crate::value;

#[derive(Clone, Dupe)]
pub struct Error {
   pub trace: Value,
   pub value: Value,
}

impl tag::DisplayTags for Error {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      use tag::Tag::{
         Newline,
         Space,
      };

      let mut head = self.trace.dupe();

      let tail = loop {
         let Ok(cons) = TryInto::<Arc<value::Cons>>::try_into(head.dupe()) else {
            break head;
         };
         let &value::Cons(ref item, ref tail) = &*cons;
         head = tail.dupe();

         let Ok(location) = TryInto::<value::Location>::try_into(item.dupe()) else {
            tags.write("self:".red());
            tags.write(Space);
            tags.write("self traceback list item not location:");
            tags.write(Space);
            item.display_tags_owned(tags);
            continue;
         };

         tags.write("while:".red().bold());
         tags.write(Space);
         tags.write("evaluating");
         tags.write(Space);
         location.display_tags_owned(tags);
         tags.write(Newline(1));
      };

      if TryInto::<value::Nil>::try_into(tail.dupe()).is_err() {
         tags.write("self:".red());
         tags.write(Space);
         tags.write("self traceback list not terminated with nil:");
         tags.write(Space);
         tail.display_tags_owned(tags);
      }

      tags.write("throw ".red().bold());
      self.value.display_tags(tags);
   }
}

impl Error {
   pub fn new(value: impl Into<Value>) -> Self {
      into!(value);

      Self {
         trace: Value::from(value::Nil),
         value,
      }
   }

   #[must_use]
   pub fn append_trace(&self, location: value::Location) -> Self {
      Self {
         trace: Value::from(Arc::new(value::Cons(
            Value::from(location),
            self.trace.dupe(),
         ))),
         value: self.value.dupe(),
      }
   }
}

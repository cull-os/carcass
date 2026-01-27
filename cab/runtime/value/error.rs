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
   pub at:    Value,
   pub value: Value,
}

impl tag::DisplayTags for Error {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      use tag::Tag::Space;

      let mut head = self.at.dupe();

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
            item.display_tags(tags);
            continue;
         };
      };

      if TryInto::<value::Nil>::try_into(tail.dupe()).is_err() {
         tags.write("self:".red());
         tags.write(Space);
         tags.write("self traceback list not terminated with nil:");
         tags.write(Space);
         tail.display_tags(tags);
      }

      tags.write("throw ".red().bold());
      self.value.display_tags(tags);
   }
}

impl Error {
   pub fn new(value: impl Into<Value>) -> Self {
      into!(value);

      Self {
         at: Value::from(value::Nil),
         value,
      }
   }

   #[must_use]
   pub fn trace(&self, at: value::Location) -> Self {
      Self {
         at:    Value::from(Arc::new(value::Cons(Value::from(at), self.at.dupe()))),
         value: self.value.dupe(),
      }
   }
}

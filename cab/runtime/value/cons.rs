use dup::Dupe;
use ust::{
   style::StyledExt as _,
   terminal::tag,
};

use crate::{
   Value,
   value,
};

#[derive(Clone, Dupe)]
pub struct Cons(pub Value, pub Value);

impl tag::DisplayTags for Cons {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      use tag::{
         Condition::{
            Broken,
            Flat,
         },
         Tag::{
            Group,
            Newline,
            Space,
         },
      };

      let &Cons(ref head, ref tail) = self;

      tags.write_with(Group(40), |tags| {
         head.display_tags(tags);

         tags.write_if(Space, Flat);
         tags.write_if(Newline(1), Broken);

         tags.write(":".style(value::STYLE_PUNCTUATION));
         tags.write(Space);

         tail.display_tags(tags);
      });
   }
}

impl From<Cons> for value::Attributes {
   fn from(cons: Cons) -> Self {
      value::attributes::new! {
         "fst": cons.0,
         "snd": cons.1,
      }
   }
}

impl TryFrom<value::Attributes> for Cons {
   type Error = ();

   fn try_from(attrs: value::Attributes) -> Result<Self, Self::Error> {
      let fst = attrs.get(&value::string::new!("fst")).ok_or(())?;
      let snd = attrs.get(&value::string::new!("snd")).ok_or(())?;
      Ok(Cons(fst.dupe(), snd.dupe()))
   }
}

#[derive(Clone, Dupe, Copy)]
pub struct Nil;

impl From<Nil> for value::Attributes {
   fn from(Nil: Nil) -> Self {
      value::attributes::new! {
         // TODO: Seems odd.
         "__nil__": Value::from(Nil)
      }
   }
}

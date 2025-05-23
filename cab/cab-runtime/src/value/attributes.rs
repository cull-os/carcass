#![allow(dead_code)]

use std::sync::Arc;

use cab_format::{
   DisplayTags,
   INDENT_WIDTH,
   Tag,
   TagCondition,
};
use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;

use super::Value;

#[derive(Clone)]
pub struct Attributes(HashTrieMap<Arc<str>, Value, FxBuildHasher>);

impl DisplayTags for Attributes {
   fn display_tags<'a>(&'a self, tags: &mut cab_format::Tags<'a>) {
      use Tag::{
         Indent,
         Newline,
         Space,
      };
      use TagCondition::{
         Always,
         Broken,
         Flat,
      };

      tags.write_with(Tag::Group(40), |tags| {
         tags.write("{");

         if !self.0.is_empty() {
            tags.write_if(Space, Flat);
         }
         tags.write_if(Newline(1), Broken);

         tags.write_if_with(Indent(INDENT_WIDTH), Broken, |tags| {
            let mut entries = self.0.iter().collect::<Vec<_>>();
            entries.sort_by_key(|&(name, _)| name);

            if !entries.is_empty() {
               tags.write_if(Space, Flat);
            }

            let mut entries = entries.into_iter().peekable();
            while let Some((name, value)) = entries.next() {
               tags.write("@");
               tags.write(&**name);
               tags.write(Space);
               tags.write("=");
               tags.write(Space);
               value.display_tags(tags);
               tags.write_if(
                  ",",
                  if entries.peek().is_some() {
                     Always
                  } else {
                     Broken
                  },
               );
               tags.write_if(Space, Flat);
            }
         });

         tags.write_if(Newline(1), Broken);
         if !self.0.is_empty() {
            tags.write_if(Space, Flat);
         }

         tags.write("}");
      });
   }
}

impl From<Attributes> for Value {
   fn from(attributes: Attributes) -> Self {
      Value::Attributes(attributes)
   }
}

impl Attributes {
   #[must_use]
   pub fn new() -> Self {
      Self(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher))
   }

   #[must_use]
   pub fn is_empty(&self) -> bool {
      self.0.is_empty()
   }
}

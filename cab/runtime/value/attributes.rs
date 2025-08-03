#![allow(dead_code)]

use dup::Dupe;
use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;
use ust::{
   INDENT_WIDTH,
   style::StyledExt as _,
   terminal::tag,
};

use super::Value;
use crate::value;

#[derive(Clone, Dupe)]
pub struct Attributes(HashTrieMap<value::SString, Value, FxBuildHasher>);

impl tag::DisplayTags for Attributes {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      use tag::{
         Condition::{
            Always,
            Broken,
            Flat,
         },
         Tag::{
            Group,
            Indent,
            Newline,
            Space,
         },
      };

      tags.write_with(Group(40), |tags| {
         tags.write("{".style(value::STYLE_PUNCTUATION));

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
               tags.write("@".style(value::STYLE_BIND_AT));
               tags.write((**name).style(value::STYLE_BIND));
               tags.write(Space);
               tags.write("=".style(value::STYLE_PUNCTUATION));
               tags.write(Space);
               value.display_tags(tags);
               tags.write_if(
                  ",".style(value::STYLE_PUNCTUATION),
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

         tags.write("}".style(value::STYLE_PUNCTUATION));
      });
   }
}

impl Attributes {
   #[must_use]
   pub fn new() -> Self {
      Self(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher))
   }

   #[must_use]
   pub fn insert(&self, key: value::SString, value: Value) -> Self {
      Self(self.0.insert(key, value))
   }

   #[must_use]
   pub fn remove(&self, key: &value::SString) -> Self {
      Self(self.0.remove(key))
   }

   #[must_use]
   pub fn get(&self, key: &value::SString) -> Option<&Value> {
      self.0.get(key)
   }
}

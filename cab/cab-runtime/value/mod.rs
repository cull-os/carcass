use std::sync::Arc;

use cab_syntax::{
   escape_string,
   is_valid_plain_identifier,
};
use rpds::ListSync as List;
use ust::{
   INDENT_WIDTH,
   style::{
      self,
      StyledExt as _,
   },
   terminal::tag,
};
use zerocopy::{
   IntoBytes as _,
   TryFromBytes as _,
};

use crate::Code;

mod attributes;
pub use attributes::Attributes;

pub mod path;
pub use path::Path;

mod thunk;
pub use thunk::Thunk;

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Value {
   Boolean(bool),

   List(List<Value>),

   Attributes(Attributes),

   Path(Path),

   Bind(Arc<str>),
   Reference(Arc<str>),
   String(Arc<str>),

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Thunk(Thunk),
   Blueprint(Arc<Code>),

   Nope,
}

impl tag::DisplayTags for Value {
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

      fn display_tags_escaped<'a>(tags: &mut tag::Tags<'a>, s: &'a str, normal: style::Style) {
         for part in escape_string(s, normal) {
            tags.write(part);
         }
      }

      match *self {
         Value::Boolean(true) => tags.write("true".magenta().bold()),
         Value::Boolean(false) => tags.write("false".magenta().bold()),

         Value::List(ref list) => {
            tags.write_with(Group(40), |tags| {
               tags.write("[".yellow());

               if !list.is_empty() {
                  tags.write_if(Space, Flat);
                  tags.write_if(Newline(1), Broken);
               }

               tags.write_if_with(Indent(INDENT_WIDTH), Broken, |tags| {
                  let mut items = list.iter().peekable();
                  while let Some(item) = items.next() {
                     item.display_tags(tags);

                     tags.write_if(
                        ",".yellow(),
                        if items.peek().is_some() {
                           Always
                        } else {
                           Broken
                        },
                     );
                     tags.write_if(Space, Flat);
                     tags.write_if(Newline(1), Broken);
                  }
               });

               tags.write("]".yellow());
            });
         },

         Value::Attributes(ref attributes) => attributes.display_tags(tags),
         Value::Path(ref path) => path.display_tags(tags),

         Value::Bind(ref identifier) => {
            tags.write("@".blue().bold());

            if is_valid_plain_identifier(identifier) {
               display_tags_escaped(tags, identifier, style::Color::Blue.fg());
            } else {
               tags.write("`".blue());
               display_tags_escaped(tags, identifier, style::Color::Blue.fg());
               tags.write("`".blue());
            }
         },

         Value::Reference(ref identifier) => {
            if is_valid_plain_identifier(identifier) {
               display_tags_escaped(tags, identifier, style::Style::default());
            } else {
               tags.write("`");
               display_tags_escaped(tags, identifier, style::Style::default());
               tags.write("`");
            }
         },

         Value::String(ref string) => {
            tags.write("\"".green());
            display_tags_escaped(tags, string, style::Color::Green.fg());
            tags.write("\"".green());
         },

         Value::Rune(ref rune) => {
            tags.write("'".green());
            // FIXME: Not all characters are valid UTF-8 byte encoded. Such as é.
            //
            // Others will silently corrupt. This only works for ASCII.
            let as_str = str::try_ref_from_bytes(rune.as_bytes()).unwrap();
            display_tags_escaped(tags, as_str, style::Color::Green.fg());
            tags.write("'".green());
         },

         Value::Integer(ref integer) => tags.write(integer.to_string().cyan().bold()),

         Value::Float(float) => tags.write(float.to_string().cyan().bold()),

         Value::Thunk(_) | Value::Blueprint(_) => tags.write("_".bright_black().bold()),

         Value::Nope => tags.write("<nope>".bright_black()),
      }
   }
}

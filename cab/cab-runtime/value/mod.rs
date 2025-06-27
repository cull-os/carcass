use std::sync::Arc;

use cab_syntax::is_valid_plain_identifier;
use ust::{
   style::StyledExt as _,
   terminal::tag,
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

   Nil,
   Cons(Arc<Value>, Arc<Value>),

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
}

impl tag::DisplayTags for Value {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      match *self {
         Value::Boolean(true) => tags.write("true".magenta().bold()),
         Value::Boolean(false) => tags.write("false".magenta().bold()),

         Value::Nil => tags.write("[]".yellow()),

         Value::Cons(ref left, ref right) => {
            left.display_tags(tags);
            tags.write(" : ");
            right.display_tags(tags);
         },

         Value::Attributes(ref attributes) => attributes.display_tags(tags),
         Value::Path(ref path) => path.display_tags(tags),

         Value::Bind(ref identifier) => {
            tags.write("@");

            if is_valid_plain_identifier(identifier) {
               // TODO: Escape.
               tags.write((**identifier).blue());
            } else {
               tags.write("`".blue());
               // TODO: Escape.
               tags.write((**identifier).blue());
               tags.write("`".blue());
            }
         },

         Value::Reference(ref identifier) => {
            if is_valid_plain_identifier(identifier) {
               // TODO: Escape.
               tags.write(&**identifier);
            } else {
               tags.write("`");
               // TODO: Escape.
               tags.write(&**identifier);
               tags.write("`");
            }
         },

         Value::String(ref string) => {
            tags.write("\"".green());
            // TODO: Escape.
            tags.write((**string).green());
            tags.write("\"".green());
         },

         Value::Rune(rune) => {
            tags.write("'".green());
            // TODO: Escape.
            tags.write(rune.to_string().green());
            tags.write("'".green());
         },

         Value::Integer(ref integer) => tags.write(integer.to_string().cyan().bold()),

         Value::Float(float) => tags.write(float.to_string().cyan().bold()),

         Value::Thunk(_) | Value::Blueprint(_) => tags.write("_".bright_black().bold()),
      }
   }
}

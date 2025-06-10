use std::sync::Arc;

mod attributes;
pub use attributes::Attributes;

mod path;
use cab_syntax::is_valid_plain_identifier;
pub use path::{
   Path,
   Root,
};
use ust::terminal::tag;

mod thunk;
pub use thunk::Thunk;

use crate::Code;

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
      use tag::Tag::Text;

      match *self {
         Value::Boolean(boolean) if boolean => tags.write("true"),
         Value::Boolean(_) => tags.write("false"),

         Value::Nil => tags.write("[]"),

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
               tags.write(&**identifier);
            } else {
               tags.write("`");
               // TODO: Escape.
               tags.write(&**identifier);
               tags.write("`");
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
            tags.write("\"");
            // TODO: Escape.
            tags.write(&**string);
            tags.write("\"");
         },

         Value::Rune(rune) => {
            tags.write("'");
            // TODO: Escape.
            tags.write(Text(rune.to_string().into()));
            tags.write("'");
         },

         Value::Integer(ref integer) => tags.write(Text(integer.to_string().into())),

         Value::Float(float) => tags.write(Text(float.to_string().into())),

         Value::Thunk(_) | Value::Blueprint(_) => tags.write("_"),
      }
   }
}

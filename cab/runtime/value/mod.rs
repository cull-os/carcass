use std::{
   marker,
   sync::Arc,
};

use cab_syntax::{
   escape,
   escape_string,
   is_valid_plain_identifier,
};
use cab_util::suffix::Arc as _;
use derive_more::{
   From,
   TryInto,
};
use dup::Dupe;
use ust::{
   style::{
      self,
      StyledExt as _,
   },
   terminal::tag,
};

use crate::Code;

pub mod attributes;
pub use attributes::Attributes;

pub mod cons;
pub use cons::{
   Cons,
   Nil,
};

pub mod error;
pub use error::Error;

pub mod integer;
pub use integer::Integer;

pub mod path;
pub use path::Path;

pub mod location;
pub use location::Location;

pub mod string;
pub use string::SString;

mod thunk;
pub use thunk::Thunk;

#[warn(variant_size_differences)]
#[derive(Clone, Dupe, From, TryInto)]
pub enum Value {
   Error(Arc<Error>),
   Location(Location),

   Boolean(bool),

   Cons(Arc<Cons>),
   Nil(Nil),

   Attributes(Attributes),

   Path(Path),

   #[from(ignore)]
   Bind(SString),
   #[from(ignore)]
   Reference(SString),

   String(SString),

   Char(char),
   Integer(Integer),
   Float(f64),

   Thunk(Thunk),

   #[from(ignore)]
   NeedsArgumentToThunk(Arc<Code>),
   #[from(ignore)]
   Thunkable(Arc<Code>),
}

pub const STYLE_ESCAPED: style::Style = style::Color::Magenta.fg().bold();
pub const STYLE_PUNCTUATION: style::Style = style::Color::Yellow.fg();
pub const STYLE_BIND: style::Style = style::Color::BrightWhite.fg();
pub const STYLE_BIND_AT: style::Style = STYLE_BIND.bold();
pub const STYLE_REFERENCE: style::Style = style::Style::new();
pub const STYLE_STRING: style::Style = style::Color::Green.fg();
pub const STYLE_CHAR: style::Style = style::Color::Green.fg();
pub const STYLE_FLOAT: style::Style = style::Color::Cyan.fg().bold();
pub const STYLE_THUNK: style::Style = style::Color::BrightBlack.fg().bold();

impl tag::DisplayTags for Value {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      #[bon::builder]
      fn display_tags_escaped<'a>(
         #[builder(start_fn)] tags: &mut tag::Tags<'a>,
         #[builder(start_fn)] s: &'a str,
         #[builder(start_fn)] normal: style::Style,
         delimiter: Option<(char, &'static str)>,
      ) {
         for part in escape_string(s)
            .normal_style(normal)
            .escaped_style(STYLE_ESCAPED)
            .maybe_delimiter(delimiter)
            .call()
         {
            tags.write(part);
         }
      }

      match *self {
         Value::Error(ref error) => error.display_tags(tags),
         Value::Location(ref location) => location.display_tags(tags),

         Value::Boolean(true) => tags.write("true".magenta().bold()),
         Value::Boolean(false) => tags.write("false".magenta().bold()),

         Value::Nil(Nil) => tags.write("[]".style(STYLE_PUNCTUATION)),
         Value::Cons(ref cons) => cons.display_tags(tags),

         Value::Attributes(ref attributes) => attributes.display_tags(tags),
         Value::Path(ref path) => path.display_tags(tags),

         Value::Bind(ref identifier) => {
            tags.write("@".style(STYLE_BIND_AT));

            if is_valid_plain_identifier(identifier) {
               display_tags_escaped(tags, identifier, STYLE_BIND).call();
            } else {
               tags.write("`".style(STYLE_BIND));
               display_tags_escaped(tags, identifier, STYLE_BIND)
                  .delimiter(('`', "\\`"))
                  .call();
               tags.write("`".style(STYLE_BIND));
            }
         },

         Value::Reference(ref identifier) => {
            if is_valid_plain_identifier(identifier) {
               display_tags_escaped(tags, identifier, STYLE_REFERENCE).call();
            } else {
               tags.write("`");
               display_tags_escaped(tags, identifier, STYLE_REFERENCE)
                  .delimiter(('`', "\\`"))
                  .call();
               tags.write("`");
            }
         },

         Value::String(ref string) => {
            tags.write("\"".style(STYLE_STRING));
            display_tags_escaped(tags, string, STYLE_STRING)
               .delimiter(('"', "\\\""))
               .call();
            tags.write("\"".style(STYLE_STRING));
         },

         Value::Char(char) => {
            tags.write("'".style(STYLE_CHAR));
            match escape(char).is_first(true).delimiter(('\'', "\\'")).call() {
               Some(escaped) => tags.write(escaped.style(STYLE_ESCAPED)),
               None => tags.write(char.to_string().style(STYLE_CHAR)),
            }
            tags.write("'".style(STYLE_CHAR));
         },

         Value::Integer(ref integer) => integer.display_tags(tags),

         Value::Float(float) => tags.write(float.to_string().style(STYLE_FLOAT)),

         Value::Thunk(_) | Value::NeedsArgumentToThunk(..) | Value::Thunkable(_) => {
            tags.write("_".style(STYLE_THUNK));
         },
      }
   }
}

impl From<&Value> for Attributes {
   fn from(_value: &Value) -> Self {
      todo!()
   }
}

impl Value {
   #[must_use]
   pub fn typed<T: Dupe>(self) -> Typed<T>
   where
      Self: TryInto<T>,
   {
      Typed {
         value:  self,
         _typed: marker::PhantomData,
      }
   }

   #[must_use]
   pub fn equals(left: &Value, right: &Value) -> (bool, Attributes) {
      match (left, right) {
         (left @ &Self::Bind(ref left_identifier), right @ &Self::Bind(ref right_identifier)) => {
            (
               true,
               attributes::new! {}
                  .insert(left_identifier.dupe(), right.dupe())
                  .insert(right_identifier.dupe(), left.dupe()),
            )
         },
         (&Self::Bind(ref identifier), value) | (value, &Self::Bind(ref identifier)) => {
            (
               true,
               attributes::new! {}.insert(identifier.dupe(), value.dupe()),
            )
         },
         (left, right) => {
            Attributes::equals(
               &Into::<Attributes>::into(left),
               &Into::<Attributes>::into(right),
            )
         },
      }
   }
}

#[derive(Clone, Dupe)]
#[repr(transparent)]
pub struct Typed<T: Dupe>
where
   Value: TryInto<T>,
{
   value:  Value,
   _typed: marker::PhantomData<T>,
}

impl<T: Dupe> From<Value> for Typed<T>
where
   Value: TryInto<T>,
{
   fn from(value: Value) -> Self {
      Self {
         value,
         _typed: marker::PhantomData,
      }
   }
}

impl<T: Dupe> Typed<T>
where
   Value: TryInto<T>,
{
   pub fn must(self) -> Result<T, Value> {
      self.value.try_into().map_err(|_| {
         Value::from(Error::new(string::new!("TODO better expected type error")).arc())
      })
   }
}

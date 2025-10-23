use std::sync::Arc;

use cab_syntax::{
   escape,
   escape_string,
   is_valid_plain_identifier,
};
use cab_util::into;
use derive_more::From;
use dup::Dupe;
use rpds::ListSync as List;
use ust::{
   INDENT_WIDTH,
   style::{
      self,
      StyledExt as _,
   },
   terminal::tag,
};

use crate::Code;

pub mod attributes;
pub use attributes::Attributes;

pub mod path;
pub use path::Path;

pub mod string;
pub use string::SString;

mod thunk;
pub use thunk::Thunk;

#[warn(variant_size_differences)]
#[derive(Clone, Dupe, From)]
pub enum Value {
   Error(Arc<Value>),

   Boolean(bool),

   List(List<Value>),
   Attributes(Attributes),

   Path(Path),

   #[from(ignore)]
   Bind(SString),
   #[from(ignore)]
   Reference(SString),

   String(SString),

   Char(char),
   Integer(Arc<num::BigInt>),
   Float(f64),

   Thunk(Thunk), // Unused for now.

   #[from(ignore)]
   Suspend(Arc<Code>),

   #[from(ignore)]
   Lambda(Arc<Code>),
}

pub const STYLE_ESCAPED: style::Style = style::Color::Magenta.fg().bold();
pub const STYLE_PUNCTUATION: style::Style = style::Color::Yellow.fg();
pub const STYLE_BIND: style::Style = style::Color::BrightWhite.fg();
pub const STYLE_BIND_AT: style::Style = STYLE_BIND.bold();
pub const STYLE_REFERENCE: style::Style = style::Style::new();
pub const STYLE_STRING: style::Style = style::Color::Green.fg();
pub const STYLE_CHAR: style::Style = style::Color::Green.fg();
pub const STYLE_INTEGER: style::Style = style::Color::Cyan.fg().bold();
pub const STYLE_FLOAT: style::Style = style::Color::Cyan.fg().bold();
pub const STYLE_THUNK: style::Style = style::Color::BrightBlack.fg().bold();

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
         Value::Error(ref value) => {
            tags.write("throw ".red().bold());

            // No need to add special logic,
            // only `Error` can generate tags that can
            // cause parse errors when nested right now.
            //
            // Maybe in the future.
            if let Value::Error(_) = **value {
               tags.write("(".yellow());
            }

            value.display_tags(tags);

            if let Value::Error(_) = **value {
               tags.write(")".yellow());
            }
         },

         Value::Boolean(true) => tags.write("true".magenta().bold()),
         Value::Boolean(false) => tags.write("false".magenta().bold()),

         Value::List(ref list) => {
            tags.write_with(Group(40), |tags| {
               tags.write("[".style(STYLE_PUNCTUATION));

               if !list.is_empty() {
                  tags.write_if(Space, Flat);
                  tags.write_if(Newline(1), Broken);
               }

               tags.write_if_with(Indent(INDENT_WIDTH), Broken, |tags| {
                  let mut items = list.iter().peekable();
                  while let Some(item) = items.next() {
                     item.display_tags(tags);

                     tags.write_if(
                        ",".style(STYLE_PUNCTUATION),
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

               tags.write("]".style(STYLE_PUNCTUATION));
            });
         },

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
            match escape(char).delimiter(('\'', "\\'")).call() {
               Some(escaped) => tags.write(escaped.style(STYLE_ESCAPED)),
               None => tags.write(char.to_string().style(STYLE_CHAR)),
            }
            tags.write("'".style(STYLE_CHAR));
         },

         Value::Integer(ref integer) => tags.write(integer.to_string().style(STYLE_INTEGER)),

         Value::Float(float) => tags.write(float.to_string().style(STYLE_FLOAT)),

         Value::Thunk(_) | Value::Suspend(_) | Value::Lambda(_) => {
            tags.write("_".style(STYLE_THUNK));
         },
      }
   }
}

impl Value {
   #[must_use]
   pub fn error(inner: impl Into<Self>) -> Self {
      into!(inner);

      // TODO: Definitely won't stay like this for long.
      Self::Error(Arc::new(Self::from(attributes::new! {
         "__error__": inner,
      })))
   }
}

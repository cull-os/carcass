use std::sync::Arc;

use cab_syntax::{
   escape,
   escape_string,
   is_valid_plain_identifier,
};
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

mod attributes;
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
   Thunkprint(Arc<Code>),

   #[from(ignore)]
   Lambda(Arc<Code>),
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

      const ESCAPED_STYLE: style::Style = style::Color::Magenta.fg().bold();

      #[bon::builder]
      fn display_tags_escaped<'a>(
         #[builder(start_fn)] tags: &mut tag::Tags<'a>,
         #[builder(start_fn)] s: &'a str,
         #[builder(start_fn)] normal: style::Style,
         delimiter: Option<(char, &'static str)>,
      ) {
         for part in escape_string(s)
            .normal_style(normal)
            .escaped_style(ESCAPED_STYLE)
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
               display_tags_escaped(tags, identifier, style::Color::Blue.fg()).call();
            } else {
               tags.write("`".blue());
               display_tags_escaped(tags, identifier, style::Color::Blue.fg())
                  .delimiter(('`', "\\`"))
                  .call();
               tags.write("`".blue());
            }
         },

         Value::Reference(ref identifier) => {
            if is_valid_plain_identifier(identifier) {
               display_tags_escaped(tags, identifier, style::Style::default()).call();
            } else {
               tags.write("`");
               display_tags_escaped(tags, identifier, style::Style::default())
                  .delimiter(('`', "\\`"))
                  .call();
               tags.write("`");
            }
         },

         Value::String(ref string) => {
            tags.write("\"".green());
            display_tags_escaped(tags, string, style::Color::Green.fg())
               .delimiter(('"', "\\\""))
               .call();
            tags.write("\"".green());
         },

         Value::Char(char) => {
            tags.write("'".green());
            match escape(char).delimiter(('\'', "\\'")).call() {
               Some(escaped) => tags.write(escaped.style(ESCAPED_STYLE)),
               None => tags.write(char.to_string().green()),
            }
            tags.write("'".green());
         },

         Value::Integer(ref integer) => tags.write(integer.to_string().cyan().bold()),

         Value::Float(float) => tags.write(float.to_string().cyan().bold()),

         Value::Thunk(_) | Value::Thunkprint(_) | Value::Lambda(_) => {
            tags.write("_".bright_black().bold());
         },
      }
   }
}

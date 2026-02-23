use std::sync::Arc;

use cab_util::suffix::Arc as _;
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
pub struct Integer(Arc<num::BigInt>);

impl tag::DisplayTags for Integer {
   fn display_tags<'a>(&'a self, tags: &mut tag::Tags<'a>) {
      tags.write(self.0.to_string().cyan().bold());
   }
}

impl From<num::BigInt> for Integer {
   fn from(integer: num::BigInt) -> Self {
      Self(integer.arc())
   }
}

// FIXME: Incredibly sloppy code. Also, negatives? Lol, lmao even.
impl From<Integer> for value::Attributes {
   fn from(val: Integer) -> Self {
      let mut number = (*val.0).clone();

      // TODO: Better zero and succ.
      let mut attributes =
         value::attributes::new! { "__zero__": Value::from(value::attributes::new! {}) };

      while number != 0_u32.into() {
         attributes = value::attributes::new! { "__succ__": Value::from(attributes) };
         number -= 1_u32;
      }

      attributes
   }
}

impl TryFrom<value::Attributes> for Integer {
   type Error = ();

   fn try_from(_attributes: value::Attributes) -> Result<Self, Self::Error> {
      todo!()
   }
}

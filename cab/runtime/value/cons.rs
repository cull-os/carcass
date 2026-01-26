use dup::Dupe;

use crate::{
   Value,
   value,
};

#[derive(Clone, Dupe)]
pub struct Cons(pub Value, pub Value);

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

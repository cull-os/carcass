use std::sync::Arc;

use crate::Code;

#[warn(variant_size_differences)]
#[derive(Clone)]
pub enum Constant {
   Nil,

   Rune(char),
   Integer(num::BigInt),
   Float(f64),

   Blueprint(Arc<Code>),
}

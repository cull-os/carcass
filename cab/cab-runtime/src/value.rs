#[warn(variant_size_differences)]
#[derive(Debug, Clone)]
pub enum Value {
   Rune(char),
   Integer(num::BigInt),
   Float(f64),
}

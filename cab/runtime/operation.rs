use crate::{
   ByteIndex,
   ValueIndex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
   Push,
   Pop,

   Swap,

   Jump,
   JumpIf,

   Force,

   ScopeStart,
   ScopeEnd,
   ScopePush,
   ScopeSwap,

   Interpolate,

   Resolve,

   AssertBoolean,

   // PREFIX
   Swwallation,
   Negation,

   Not,

   // INFIX
   Concat,
   Construct,

   Call,

   Update,

   LessOrEqual,
   Less,
   MoreOrEqual,
   More,

   Equal,

   All,
   Any,

   Addition,
   Subtraction,
   Multiplication,
   Power,
   Division,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argument {
   U16(u16),
   U64(u64),
   ValueIndex(ValueIndex),
   ByteIndex(ByteIndex),
}

impl Argument {
   #[must_use]
   pub fn as_u16(&self) -> Option<u16> {
      if let &Self::U16(u16) = self {
         Some(u16)
      } else {
         None
      }
   }

   #[must_use]
   pub fn as_u64(&self) -> Option<u64> {
      if let &Self::U64(u64) = self {
         Some(u64)
      } else {
         None
      }
   }

   #[must_use]
   pub fn as_value_index(&self) -> Option<ValueIndex> {
      if let &Self::ValueIndex(index) = self {
         Some(index)
      } else {
         None
      }
   }

   #[must_use]
   pub fn as_byte_index(&self) -> Option<ByteIndex> {
      if let &Self::ByteIndex(index) = self {
         Some(index)
      } else {
         None
      }
   }
}

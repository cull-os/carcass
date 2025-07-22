use crate::{
   ByteIndex,
   ValueIndex,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
   Return,

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

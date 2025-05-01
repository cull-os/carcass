/// {n} = stack indexing, from the end.
///
/// [n:type] = instruction indexing, index 0 is right after the documented
/// instruction.
#[derive(num_enum::TryFromPrimitive)]
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

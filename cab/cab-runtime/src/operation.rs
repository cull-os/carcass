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

   IslandHeaderInterpolate,
   Island,
   PathInterpolate,
   BindInterpolate,
   IdentifierInterpolate,

   Resolve,

   // PREFIX
   Swwallation,
   Negation,

   Not,

   // INFIX
   Sequence,

   Concat,
   Construct,

   Update,

   LessOrEqual,
   Less,
   MoreOrEqual,
   More,

   Equal,

   And,
   Or,
   Implication,

   All,
   Any,

   Addition,
   Subtraction,
   Multiplication,
   Power,
   Division,
}

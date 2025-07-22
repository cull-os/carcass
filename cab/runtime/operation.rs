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
   U64,
   ValueIndex,

   U16,
   ByteIndex,
}

impl Operation {
   #[must_use]
   pub fn arguments(self) -> &'static [Argument] {
      use Argument::{
         ByteIndex,
         U16,
         U64,
         ValueIndex,
      };

      match self {
         Operation::Return => &[],

         Operation::Push => &[ValueIndex],
         Operation::Pop => &[],

         Operation::Swap => &[],

         Operation::Jump | Operation::JumpIf => &[ByteIndex],

         Operation::Force => &[],

         Operation::ScopeStart
         | Operation::ScopeEnd
         | Operation::ScopePush
         | Operation::ScopeSwap => &[],

         Operation::Interpolate => &[U64],

         Operation::Resolve => &[],

         Operation::AssertBoolean => &[],

         Operation::Swwallation | Operation::Negation => &[],

         Operation::Not => &[],

         Operation::Concat | Operation::Construct => &[],

         Operation::Update
         | Operation::LessOrEqual
         | Operation::Less
         | Operation::MoreOrEqual
         | Operation::More => &[],

         Operation::Equal => &[],

         Operation::All | Operation::Any => &[],

         Operation::Addition
         | Operation::Subtraction
         | Operation::Multiplication
         | Operation::Power
         | Operation::Division => &[],
      }
   }
}

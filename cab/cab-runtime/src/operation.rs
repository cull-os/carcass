#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
   // Small numbers represented as 1 u8 in vu128 are [0, 2**7) so starting the operation at that
   // increses our chances of Code::read_operation panicking if we ever write wrong code.
   Value = 2u8.pow(7),

   Return,

   Force,
   ForceAndCollectScope,
   Scope,

   IslandHeaderInterpolate,
   Island,
   PathInterpolate,
   BindInterpolate,
   IdentifierInterpolate,

   GetLocal,

   // PREFIX
   Swwallation,
   Negation,

   Not,

   Try,

   // INFIX
   Same,
   Sequence,

   Apply,

   Concat,
   Construct,

   Select,
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

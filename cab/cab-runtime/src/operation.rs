/// {n} = stack indexing, from the end.
///
/// [n:type] = instruction indexing, index 0 is right after the documented
/// instruction.
#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum Operation {
   // Small numbers represented as 1 u8 in vu128 are [0, 2**7) so starting the operation at that
   // increses our chances of Code::read_operation panicking if we ever write wrong code.
   /// Pushes the value with the index stored at [0:u64] onto the stack.
   Push = 2u8.pow(7),

   /// Discards the current frame.
   Return,

   /// Forces {0}.
   Force,

   /// Pushes the current scope to the stack.
   PushScope,

   /// Creates a new scope.
   ScopeStart,
   /// Ends the current scope.
   ScopeEnd,

   IslandHeaderInterpolate,
   Island,
   PathInterpolate,
   BindInterpolate,
   IdentifierInterpolate,

   /// Fetches a
   GetLocal,

   /// Jumps to {1} if {0}.
   JumpIf,

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

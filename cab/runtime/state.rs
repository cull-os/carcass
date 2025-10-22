use cab_syntax::ParseOracle;
use rpds::ListSync as List;

use crate::{
   CompileOracle,
   Value,
   value,
};

pub type Scopes = List<value::Attributes>;

pub struct State {
   _parse_oracle:    ParseOracle,
   _compile_oraclce: CompileOracle,

   // TODO: Make the runtime multi-threaded.
   pub stack:  Vec<Value>,
   pub scopes: Scopes,
}

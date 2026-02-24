use cab_syntax::ParseOracle;
use rpds::ListSync as List;

use crate::{
   CompileOracle,
   value,
};

pub type ScopeId = u64;
pub type Scope = (ScopeId, value::Attributes);
pub type Scopes = List<Scope>;

pub struct State {
   pub parse_oracle:   ParseOracle,
   pub compile_oracle: CompileOracle,
}

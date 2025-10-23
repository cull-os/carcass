use cab_syntax::ParseOracle;
use rpds::ListSync as List;

use crate::{
   CompileOracle,
   value,
};

pub type Scopes = List<value::Attributes>;

pub struct State {
   pub parse_oracle:   ParseOracle,
   pub compile_oracle: CompileOracle,
}

use cab_syntax::ParseOracle;

use crate::CompileOracle;

pub struct State {
   pub parse_oracle:   ParseOracle,
   pub compile_oracle: CompileOracle,
}

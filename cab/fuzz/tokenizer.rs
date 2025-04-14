#![no_main]

use std::hint;

use cab::syntax;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
   for token in syntax::tokenize(data) {
      hint::black_box(token);
   }
});

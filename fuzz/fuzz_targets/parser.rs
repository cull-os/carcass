#![no_main]

use std::hint::black_box;

use cab::syntax::parse;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    black_box(parse(data));
});

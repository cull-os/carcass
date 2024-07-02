#![no_main]

use std::hint::black_box;

use cab_syntax::Tokenizer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    for token in Tokenizer::new(data) {
        black_box(token);
    }
});

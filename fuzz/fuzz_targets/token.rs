#![no_main]

use std::hint::black_box;

use cab_syntax::Tokenizer;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    Tokenizer::new(data).for_each(|token| {
        black_box(token);
    })
});

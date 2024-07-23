#![no_main]

use std::hint::black_box;

use cab::syntax::tokenize;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    for token in tokenize(data) {
        black_box(token);
    }
});

#![no_main]

use std::hint;

use cab::syntax::tokenize;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    for token in tokenize(data) {
        hint::black_box(token);
    }
});

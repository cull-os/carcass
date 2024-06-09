#![no_main]

use cab_syntax::{
    Tokenizer,
    SYNTAX_COLORS,
};
use colored::{
    Colorize,
    CustomColor,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &str| {
    for token in Tokenizer::new(data) {
        let on_color = SYNTAX_COLORS[token.0 as usize];

        let color = if (0.2126 * on_color.r as f32
            + 0.7152 * on_color.g as f32
            + 0.0722 * on_color.b as f32)
            < 140.0
        {
            CustomColor::new(0xFF, 0xFF, 0xFF)
        } else {
            CustomColor::new(0, 0, 0)
        };

        print!(
            "{slice}",
            slice = token.1.on_custom_color(on_color).custom_color(color)
        )
    }
});

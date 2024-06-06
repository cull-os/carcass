use std::ptr;

use crate::SyntaxKind;

pub struct Token<'a>(SyntaxKind, &'a str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenizerContext {
    StringBody,
    StringEnd,
    Interpolation { curlybrace_count: u32 },
    InterpolationStart,
    Path,
}

#[derive(Clone, Copy)]
struct TokenizerState<'a> {
    input: &'a str,
    offset: usize,
}

impl PartialEq for TokenizerState<'_> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(self.input, other.input) && self.offset == other.offset
    }
}

impl Eq for TokenizerState<'_> {}

pub struct Tokenizer<'a> {
    context: Vec<TokenizerContext>,
    state: TokenizerState<'a>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            context: Vec::new(),
            state: TokenizerState { input, offset: 0 },
        }
    }
}

// use std::collections::VecDeque;

// use crate::{
//     kind::SyntaxKind::{
//         self,
//         *,
//     },
//     Token,
// };

// pub struct Parse {
//     node: rowan::GreenNode,
//     errors: Vec<ParseError>,
// }

// pub enum ParseError {
//     Unexpected {
//         got: rowan::TextRange,
//     },
//     WantedNotFound {
//         wanted: SyntaxKind,
//         got: Box<[SyntaxKind]>,
//         at: rowan::TextRange,
//     },

//     UnexpectedEof,
//     WantedButGotEof {
//         wanted: Box<[SyntaxKind]>,
//     },

//     DuplicatedArguments {
//         at: rowan::TextRange,
//         argument: String,
//     },

//     RecursionLimitExceeded,
// }

// pub struct Parser<'a, I: Iterator<Item = Token<'a>>> {
//     builder: rowan::GreenNodeBuilder<'static>,
//     errors: Vec<ParseError>,

//     trivia_buffer: Vec<Token<'a>>,
//     buffer: VecDeque<Token<'a>>,
//     iter: I,
//     consumed: rowan::TextSize,

//     depth: u32,
// }

// impl<'a, I: Iterator<Item = Token<'a>>> Parser<'a, I> {
//     pub fn parse(mut self, input: &str) -> Parse {
//         todo!()
//     }
// }

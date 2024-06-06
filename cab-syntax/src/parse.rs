use crate::kind::*;

pub struct Parse {
    node: rowan::GreenNode,
    errors: Vec<ParseError>,
}

pub enum ParseError {
    Unexpected {
        got: rowan::TextRange,
    },
    WantedNotFound {
        wanted: SyntaxKind,
        got: Box<[SyntaxKind]>,
        at: rowan::TextRange,
    },

    UnexpectedEof,
    WantedButGotEof {
        got: Box<[SyntaxKind]>,
    },

    DuplicatedArguments {
        at: rowan::TextRange,
        argument: String,
    },

    RecursionLimitExceeded,
}

fn parse(text: &str) -> Parse {
    struct Parser {
        tokens: Vec<(SyntaxKind, String)>,
        builder: rowan::GreenNodeBuilder<'static>,
        errors: Vec<String>,
    }

    impl Parser {
        fn parse(mut self) -> Parse {
            self.builder.start_node(NODE_ROOT.into());

            loop {}
        }
    }
}

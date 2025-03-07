use std::{
    fmt::Write as _,
    result,
    sync::Arc,
};

use cab_why::{
    IntoSize,
    Report,
    ReportSeverity,
    Size,
    Span,
};
use enumset::EnumSet;
use peekmore::{
    PeekMore as _,
    PeekMoreIterator as PeekMore,
};

use crate::{
    Kind::{
        self,
        *,
    },
    green,
    node,
    red,
};

/// A parse result that contains a node, a [`node::Expression`] and a
/// list of [`Report`]s.
#[derive(Debug)]
pub struct Parse {
    /// The underlying node.
    pub node: red::Node,

    /// The [`node::Expression`].
    pub expression: node::Expression,

    /// Issues reported during parsing.
    pub reports: Vec<Report>,
}

impl Parse {
    /// Returns [`Ok`] with the [`node::Expression`] if there are no error
    /// level or above reports, returns [`Err`] with the list of reports
    /// otherwise.
    pub fn result(self) -> result::Result<node::Expression, Vec<Report>> {
        if self
            .reports
            .iter()
            .all(|report| report.severity < ReportSeverity::Error)
        {
            Ok(self.expression)
        } else {
            Err(self.reports)
        }
    }
}

/// A parse oracle that holds a cache for token deduplication.
pub struct Oracle {
    cache: green::NodeCache,
}

/// Returns a fresh parse oracle with an empty cache.
pub fn oracle() -> Oracle {
    Oracle {
        cache: green::NodeCache::from_interner(green::interner()),
    }
}

impl Oracle {
    pub fn parse<'a>(&self, tokens: impl Iterator<Item = (Kind, &'a str)>) -> Parse {
        let mut noder = Noder::with_interner_and_tokens(Arc::clone(self.cache.interner()), tokens);

        noder.node(NODE_ROOT, |this| {
            this.node_expression(EnumSet::empty());
            this.next_expect(EnumSet::empty(), EnumSet::empty());
        });

        let (green_node, _) = noder.builder.finish();

        let node = red::Node::new_root_with_resolver(green_node, Arc::clone(self.cache.interner()));

        let expression = node
            .first_child()
            .and_then(|node| node::Expression::try_from(node.clone()).ok())
            .unwrap();

        noder.reports.retain({
            let mut last_span = None;

            move |report| {
                let Some(start) = report.labels.iter().map(|label| label.span.start).min() else {
                    return true;
                };

                if last_span != Some(start) {
                    last_span = Some(start);
                    true
                } else {
                    false
                }
            }
        });

        expression.as_ref().validate(&mut noder.reports);

        Parse {
            node,
            expression,
            reports: noder.reports,
        }
    }
}

fn unexpected(got: Option<Kind>, mut expected: EnumSet<Kind>, span: Span) -> Report {
    let report = match got {
        Some(kind) => Report::error(format!("didn't expect {kind}")),
        None => Report::error("didn't expect end of file"),
    };

    let mut reason = if expected.is_empty() {
        return report.primary(span, "expected end of file");
    } else {
        String::from("expected ")
    };

    if expected.is_superset(Kind::EXPRESSIONS) {
        expected.remove_all(Kind::EXPRESSIONS);

        let separator = match expected.len() {
            0 => "",
            1 => " or ",
            2.. => ", ",
        };

        write!(reason, "an expression{separator}").ok();
    }

    if expected.is_superset(Kind::IDENTIFIERS) {
        expected.remove(TOKEN_IDENTIFIER_START);
    }

    for (index, item) in expected.into_iter().enumerate() {
        let position = index + 1;

        let separator = match position {
            position if expected.len() == position => "",
            position if expected.len() == position + 1 => " or ",
            _ => ", ",
        };

        write!(reason, "{item}{separator}").ok();
    }

    if let Some(got) = got {
        write!(reason, ", got {got}").ok();
    } else {
        write!(reason, ", reached end of file").ok();
    }

    report.primary(span, reason)
}

type Result<T> = result::Result<T, ()>;

struct Noder<'a, I: Iterator<Item = (Kind, &'a str)>> {
    builder: green::NodeBuilder,

    tokens: PeekMore<I>,
    reports: Vec<Report>,

    offset: Size,
}

impl<'a, I: Iterator<Item = (Kind, &'a str)>> Noder<'a, I> {
    fn with_interner_and_tokens(interner: green::Interner, tokens: I) -> Self {
        Self {
            builder: green::NodeBuilder::from_interner(interner),

            tokens: tokens.peekmore(),
            reports: Vec::new(),

            offset: Size::new(0u32),
        }
    }

    fn checkpoint(&mut self) -> green::Checkpoint {
        self.next_while_trivia();
        self.builder.checkpoint()
    }

    fn node<T>(&mut self, kind: Kind, closure: impl FnOnce(&mut Self) -> T) -> T {
        self.builder.start_node(kind);

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn node_from<T>(&mut self, checkpoint: green::Checkpoint, kind: Kind, closure: impl FnOnce(&mut Self) -> T) -> T {
        self.builder.start_node_at(checkpoint, kind);

        let result = closure(self);

        self.builder.finish_node();
        result
    }

    fn peek_direct(&mut self) -> Option<Kind> {
        self.tokens.peek().map(|&(kind, _)| kind)
    }

    fn peek_nth(&mut self, n: usize) -> Option<Kind> {
        let mut peek_index: usize = 0;
        let mut index: usize = 0;

        loop {
            let &(kind, _) = self.tokens.peek_nth(peek_index)?;

            if index >= n && !kind.is_trivia() {
                return Some(kind);
            }

            peek_index += 1;

            if !kind.is_trivia() {
                index += 1;
            }
        }
    }

    fn peek(&mut self) -> Option<Kind> {
        self.peek_nth(0)
    }

    fn next_direct(&mut self) -> Result<Kind> {
        match self.tokens.next() {
            Some((kind, slice)) => {
                self.offset += slice.size();
                self.builder.token(kind, slice);

                Ok(kind)
            },

            None => {
                self.reports
                    .push(unexpected(None, EnumSet::empty(), Span::empty(self.offset)));

                Err(())
            },
        }
    }

    fn next_direct_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) {
        while self.peek_direct().is_some_and(&mut predicate) {
            self.next_direct().unwrap();
        }
    }

    fn next_while_trivia(&mut self) {
        self.next_direct_while(Kind::is_trivia)
    }

    fn next(&mut self) -> Result<Kind> {
        self.next_while_trivia();
        self.next_direct()
    }

    fn next_if(&mut self, expected: Kind) -> bool {
        let condition = self.peek() == Some(expected);

        if condition {
            self.next().unwrap();
        }

        condition
    }

    fn next_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) -> Span {
        let start = self.offset;

        while self.peek().is_some_and(&mut predicate) {
            self.next().unwrap();
        }

        Span::new(start, self.offset)
    }

    fn next_expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> Option<Kind> {
        let expected_at = self.checkpoint();

        match self.peek() {
            None if expected.is_empty() => None,
            Some(next) if expected.contains(next) => Some(self.next().unwrap()),

            unexpected => {
                let unexpected_span = self.next_while(|next| !(until | expected).contains(next));

                self.node_from(expected_at, NODE_ERROR, |_| {});

                self.reports
                    .push(self::unexpected(unexpected, expected, unexpected_span));

                let next = self.peek()?;

                if expected.contains(next) {
                    Some(self.next().unwrap())
                } else {
                    None
                }
            },
        }
    }

    fn node_parenthesis(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_PARENTHESIS, |this| {
            this.next_expect(
                TOKEN_LEFT_PARENTHESIS.into(),
                until | Kind::EXPRESSIONS | TOKEN_RIGHT_PARENTHESIS,
            );

            if this.peek().is_some_and(|kind| kind != TOKEN_RIGHT_PARENTHESIS) {
                this.node_expression(until | TOKEN_RIGHT_PARENTHESIS);
            }

            this.next_if(TOKEN_RIGHT_PARENTHESIS);
        });
    }

    fn node_list(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_LIST, |this| {
            this.next_expect(
                TOKEN_LEFT_BRACKET.into(),
                until | Kind::EXPRESSIONS | TOKEN_RIGHT_BRACKET,
            );

            if this.peek().is_some_and(|kind| kind != TOKEN_RIGHT_BRACKET) {
                this.node_expression(until | TOKEN_RIGHT_BRACKET);
            }

            this.next_if(TOKEN_RIGHT_BRACKET);
        });
    }

    fn node_attribute_list(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_ATTRIBUTE_LIST, |this| {
            this.next_expect(
                TOKEN_LEFT_CURLYBRACE.into(),
                until | Kind::EXPRESSIONS | TOKEN_RIGHT_CURLYBRACE,
            );

            if this.peek().is_some_and(|kind| kind != TOKEN_RIGHT_CURLYBRACE) {
                this.node_expression(until | TOKEN_RIGHT_CURLYBRACE);
            }

            this.next_if(TOKEN_RIGHT_CURLYBRACE);
        });
    }

    fn node_bind(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_BIND, |this| {
            this.next_expect(TOKEN_AT.into(), Kind::IDENTIFIERS);

            this.next_while_trivia();
            this.node_expression_single(until);
        });
    }

    fn node_identifier(&mut self, until: EnumSet<Kind>) {
        if self.peek() == Some(TOKEN_IDENTIFIER_START) {
            self.node_delimited();
        } else {
            self.node(NODE_IDENTIFIER, |this| {
                this.next_expect(Kind::IDENTIFIERS, until);
            });
        }
    }

    fn node_delimited(&mut self) {
        let start_of_delimited = self.checkpoint();

        let (node, end) = self.next().unwrap().as_node_and_closing().unwrap();

        self.node_from(start_of_delimited, node, |this| {
            loop {
                match this.peek() {
                    Some(TOKEN_CONTENT) => {
                        this.next_direct().unwrap();
                    },

                    Some(TOKEN_INTERPOLATION_START) => {
                        this.node_interpolation();
                    },

                    Some(other) if other == end => {
                        this.next_direct().unwrap();
                        break;
                    },

                    Some(_) => {
                        // Sometimes recoverably parsing interpolation leaves us unwanted tokens. It
                        // is not worth it trying to node it correctly without a big rewrite, so
                        // just consume them.
                        this.next_direct().unwrap();
                    },

                    None => {
                        this.reports
                            .push(unexpected(None, TOKEN_CONTENT | end, Span::empty(this.offset)));
                        break;
                    },
                }
            }
        });

        // Treat expressions such as <foo>/bar as applciations. The evaluator will
        // special case on island<->path applications that contain literals to be path
        // accesses as islands are just virtual path roots anyway. Normally you cannot
        // use an island as a functor, I must say. That will be a hard error.
        if node == NODE_ISLAND && self.peek_direct() == Some(TOKEN_PATH_START) {
            self.node_from(start_of_delimited, NODE_INFIX_OPERATION, |this| {
                this.node_delimited();
            });
        }
    }

    fn node_interpolation(&mut self) {
        self.node(NODE_INTERPOLATION, |this| {
            this.next_expect(TOKEN_INTERPOLATION_START.into(), EnumSet::empty());

            this.node_expression(TOKEN_INTERPOLATION_END.into());

            this.next_expect(TOKEN_INTERPOLATION_END.into(), EnumSet::empty());
        });
    }

    fn node_integer(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_INTEGER, |this| {
            this.next_expect(TOKEN_INTEGER.into(), until);
        });
    }

    fn node_float(&mut self, until: EnumSet<Kind>) {
        self.node(NODE_FLOAT, |this| {
            this.next_expect(TOKEN_FLOAT.into(), until);
        });
    }

    fn node_if(&mut self, until: EnumSet<Kind>) {
        let then_else_binding_power = node::InfixOperator::Same.binding_power().0 + 1;

        self.node(NODE_IF, |this| {
            this.next_expect(
                TOKEN_LITERAL_IF.into(),
                until | Kind::EXPRESSIONS | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
            );

            this.node_expression_binding_power(
                then_else_binding_power,
                until | TOKEN_LITERAL_THEN | TOKEN_LITERAL_ELSE,
            );

            this.next_expect(TOKEN_LITERAL_THEN.into(), until | TOKEN_LITERAL_ELSE);

            this.node_expression_binding_power(then_else_binding_power, until | TOKEN_LITERAL_ELSE);

            this.node_expression_binding_power(then_else_binding_power, until);
        });
    }

    fn node_expression_single(&mut self, until: EnumSet<Kind>) {
        let expected_at = self.checkpoint();

        match self.peek() {
            Some(TOKEN_LEFT_PARENTHESIS) => self.node_parenthesis(until),

            Some(TOKEN_LEFT_BRACKET) => self.node_list(until),

            Some(TOKEN_LEFT_CURLYBRACE) => self.node_attribute_list(until),

            Some(TOKEN_AT) => self.node_bind(until),

            Some(next) if Kind::IDENTIFIERS.contains(next) => self.node_identifier(until),

            Some(TOKEN_PATH_START | TOKEN_STRING_START | TOKEN_RUNE_START | TOKEN_ISLAND_START) => {
                self.node_delimited()
            },

            Some(TOKEN_INTEGER) => self.node_integer(until),
            Some(TOKEN_FLOAT) => self.node_float(until),

            Some(TOKEN_LITERAL_IF) => self.node_if(until),

            unexpected => {
                // Consume until the next token is either the limit, an
                // expression token or an operator.
                let unexpected_span = self.next_while(|kind| {
                    !((until | Kind::EXPRESSIONS).contains(kind)
                        || node::PrefixOperator::try_from(kind).is_ok()
                        || node::InfixOperator::try_from(kind).is_ok_and(|operator| operator.is_token_owning())
                        || node::SuffixOperator::try_from(kind).is_ok())
                });

                self.node_from(expected_at, NODE_ERROR, |_| {});

                self.reports
                    .push(self::unexpected(unexpected, Kind::EXPRESSIONS, unexpected_span));
            },
        }
    }

    fn node_expression_binding_power(&mut self, minimum_power: u16, until: EnumSet<Kind>) {
        let start_of_expression = self.checkpoint();

        if let Some(operator) = self.peek().and_then(|kind| node::PrefixOperator::try_from(kind).ok()) {
            let ((), right_power) = operator.binding_power();

            self.node(NODE_PREFIX_OPERATION, |this| {
                this.next().unwrap();
                this.node_expression_binding_power(right_power, until);
            });
        } else {
            self.node_expression_single(until);
        }

        while let Some(operator) = self.peek().and_then(|kind| node::InfixOperator::try_from(kind).ok()) {
            let (left_power, right_power) = operator.binding_power();
            if left_power < minimum_power {
                break;
            }

            let operator_token = operator.is_token_owning().then(|| self.next().unwrap());

            // Handle suffix-able infix operators. Not for purely suffix operators.
            if operator_token.is_some()
                && node::SuffixOperator::try_from(operator_token.unwrap()).is_ok()
                && self.peek().is_none_or(|kind| !Kind::EXPRESSIONS.contains(kind))
            {
                self.node_from(start_of_expression, NODE_SUFFIX_OPERATION, |_| {});
            } else {
                self.node_from(start_of_expression, NODE_INFIX_OPERATION, |this| {
                    this.node_expression_binding_power(right_power, until);
                });
            }
        }
    }

    fn node_expression(&mut self, until: EnumSet<Kind>) {
        self.node_expression_binding_power(0, until);
    }
}

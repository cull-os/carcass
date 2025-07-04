use std::fmt::Write as _;

use cab_error::{
   Contextful as _,
   Result,
   bail,
};
use cab_span::{
   IntoSize as _,
   Size,
   Span,
};
use dup::Dupe as _;
use enumset::EnumSet;
use peekmore::{
   PeekMore as _,
   PeekMoreIterator as PeekMore,
};
use ust::{
   Display,
   Write,
   report::{
      self,
      Report,
   },
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
   /// The [`node::Expression`].
   pub expression: node::Expression,

   /// The underlying node.
   pub node: red::Node,

   /// Issues reported during parsing.
   pub reports: Vec<Report>,
}

impl Parse {
   pub fn extractlnln(
      self,
      writer: &mut impl Write,
      location: &impl Display,
      source: &report::PositionStr<'_>,
   ) -> Result<node::Expression> {
      let mut fail = 0;

      for report in self.reports {
         fail += usize::from(report.severity >= report::Severity::Error);

         writer
            .write_report(&report, location, source)
            .context("failed to write report")?;

         write!(writer, "\n\n").context("failed to write report")?;
      }

      if fail > 0 {
         bail!("parsing failed due to {fail} previous error(s)");
      }

      Ok(self.expression)
   }
}

/// A parse oracle that holds a cache for token deduplication.
pub struct ParseOracle {
   cache: green::NodeCache,
}

impl ParseOracle {
   /// Returns a fresh parse oracle with an empty cache.
   #[must_use]
   pub fn new() -> Self {
      Self {
         cache: green::NodeCache::from_interner(green::interner()),
      }
   }

   pub fn parse<'a>(&self, tokens: impl Iterator<Item = (Kind, &'a str)>) -> Parse {
      let mut noder = Noder::with_interner_and_tokens(self.cache.interner().dupe(), tokens);

      noder
         .node(NODE_PARSE_ROOT)
         .with(|this| {
            this.node_expression(EnumSet::empty());
            this.next_expect(EnumSet::empty(), EnumSet::empty());
         })
         .call();

      let (green_node, _) = noder.builder.finish();

      let node = red::Node::new_root_with_resolver(green_node, self.cache.interner().dupe());

      let expression: node::ExpressionRef<'_> = node
         .first_child()
         .expect("noder output must contain a single parse root node")
         .try_into()
         .expect("parse root node must contain an expression");

      noder.reports.retain({
         let mut last_span = None;

         move |report| {
            let Some(start) = report.labels.iter().map(|label| label.span.start).min() else {
               return true;
            };

            if last_span == Some(start) {
               false
            } else {
               last_span = Some(start);
               true
            }
         }
      });

      expression.validate(&mut noder.reports);

      Parse {
         expression: expression.to_owned(),
         node,
         reports: noder.reports,
      }
   }
}

#[bon::builder]
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

      let _ = write!(reason, "an expression{separator}");
   }

   if expected.is_superset(Kind::IDENTIFIERS) {
      expected.remove(TOKEN_QUOTED_IDENTIFIER_START);
   }

   for (index, item) in expected.into_iter().enumerate() {
      let position = index + 1;

      let separator = match position {
         position if expected.len() == position => "",
         position if expected.len() == position + 1 => " or ",
         _ => ", ",
      };

      let _ = write!(reason, "{item}{separator}");
   }

   let _ = if let Some(got) = got {
      write!(reason, ", got {got}")
   } else {
      write!(reason, ", reached end of file")
   };

   report.primary(span, reason)
}

struct Noder<'a, I: Iterator<Item = (Kind, &'a str)>> {
   builder: green::NodeBuilder,

   tokens:  PeekMore<I>,
   reports: Vec<Report>,

   offset: Size,
}

#[bon::bon]
impl<'a, I: Iterator<Item = (Kind, &'a str)>> Noder<'a, I> {
   fn with_interner_and_tokens(interner: green::Interner, tokens: I) -> Self {
      Self {
         builder: green::NodeBuilder::from_interner(interner),

         tokens:  tokens.peekmore(),
         reports: Vec::new(),

         offset: Size::new(0_u32),
      }
   }

   fn checkpoint(&mut self) -> green::Checkpoint {
      self.next_while_trivia();
      self.builder.checkpoint()
   }

   #[builder]
   #[inline]
   fn node<T>(
      &mut self,
      #[builder(start_fn)] kind: Kind,
      with: impl FnOnce(&mut Self) -> T,
      from: Option<green::Checkpoint>,
   ) -> T {
      match from {
         Some(checkpoint) => self.builder.start_node_at(checkpoint, kind),
         None => self.builder.start_node(kind),
      }

      let result = with(self);

      self.builder.finish_node();
      result
   }

   fn peek_direct(&mut self) -> Option<Kind> {
      self.tokens.peek().map(|&(kind, _)| kind)
   }

   #[expect(clippy::min_ident_chars)]
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

   fn next_direct(&mut self) -> Kind {
      match self.tokens.next() {
         Some((kind, slice)) => {
            self.offset += slice.size();
            self.builder.token(kind, slice);

            kind
         },

         None => {
            self.reports.push(
               unexpected()
                  .expected(EnumSet::empty())
                  .span(Span::empty(self.offset))
                  .call(),
            );

            unreachable!()
         },
      }
   }

   fn next_direct_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) {
      while self.peek_direct().is_some_and(&mut predicate) {
         self.next_direct();
      }
   }

   fn next_while_trivia(&mut self) {
      self.next_direct_while(Kind::is_trivia);
   }

   fn next(&mut self) -> Kind {
      self.next_while_trivia();
      self.next_direct()
   }

   fn next_if(&mut self, expected: Kind) -> bool {
      let condition = self.peek() == Some(expected);

      if condition {
         self.next();
      }

      condition
   }

   fn next_while(&mut self, mut predicate: impl FnMut(Kind) -> bool) -> Span {
      let start = self.offset;

      while self.peek().is_some_and(&mut predicate) {
         self.next();
      }

      Span::new(start, self.offset)
   }

   fn next_expect(&mut self, expected: EnumSet<Kind>, until: EnumSet<Kind>) -> Option<Kind> {
      let expected_at = self.checkpoint();

      match self.peek() {
         None if expected.is_empty() => None,
         Some(next) if expected.contains(next) => Some(self.next()),

         got => {
            let got_span = self.next_while(|next| !(until | expected).contains(next));

            self.node(NODE_ERROR).from(expected_at).with(|_| {}).call();

            self.reports.push(
               unexpected()
                  .maybe_got(got)
                  .expected(expected)
                  .span(got_span)
                  .call(),
            );

            let next = self.peek()?;

            expected.contains(next).then(|| self.next())
         },
      }
   }

   fn node_parenthesis(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_PARENTHESIS)
         .with(|this| {
            this.next_expect(
               TOKEN_PARENTHESIS_LEFT.into(),
               until | Kind::EXPRESSIONS | TOKEN_PARENTHESIS_RIGHT,
            );

            if this
               .peek()
               .is_some_and(|kind| kind != TOKEN_PARENTHESIS_RIGHT)
            {
               this.node_expression(until | TOKEN_PARENTHESIS_RIGHT);
            }

            this.next_if(TOKEN_PARENTHESIS_RIGHT);
         })
         .call();
   }

   fn node_list(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_LIST)
         .with(|this| {
            this.next_expect(
               TOKEN_BRACKET_LEFT.into(),
               until | Kind::EXPRESSIONS | TOKEN_BRACKET_RIGHT,
            );

            if this.peek().is_some_and(|kind| kind != TOKEN_BRACKET_RIGHT) {
               this.node_expression(until | TOKEN_BRACKET_RIGHT);
            }

            this.next_if(TOKEN_BRACKET_RIGHT);
         })
         .call();
   }

   fn node_attributes(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_ATTRIBUTES)
         .with(|this| {
            this.next_expect(
               TOKEN_CURLYBRACE_LEFT.into(),
               until | Kind::EXPRESSIONS | TOKEN_CURLYBRACE_RIGHT,
            );

            if this
               .peek()
               .is_some_and(|kind| kind != TOKEN_CURLYBRACE_RIGHT)
            {
               this.node_expression(until | TOKEN_CURLYBRACE_RIGHT);
            }

            this.next_if(TOKEN_CURLYBRACE_RIGHT);
         })
         .call();
   }

   fn node_path_root(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_PATH_ROOT)
         .with(|this| {
            let end = this.node_delimited();

            if end == Some(">") {
               // DONE: <root>
               return;
            }

            // DONE: <root:

            if this.next_if(TOKEN_COLON) {
               // DONE: :path
               this.node_expression_single(until | TOKEN_MORE);
            } else {
               // DONE: config
               this.node_expression_single(until | TOKEN_COLON | Kind::EXPRESSIONS | TOKEN_MORE);

               // DONE: :path
               if this.next_if(TOKEN_COLON) {
                  this.node_expression_single(until | TOKEN_MORE);
               }
            }

            // DONE: >
            this.next_expect(TOKEN_MORE.into(), until);

            // EITHER:
            // <root>
            // <root::path>
            // <root:config>
            // <root:config:path>
         })
         .call();
   }

   fn node_path(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_PATH)
         .with(|this| {
            if this.peek() == Some(TOKEN_PATH_ROOT_TYPE_START) {
               this.node_path_root(until | TOKEN_PATH_SUBPATH_START);
            }

            if this.peek() == Some(TOKEN_PATH_SUBPATH_START) {
               this.node_delimited();
            }
         })
         .call();
   }

   fn node_bind(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_BIND)
         .with(|this| {
            this.next_expect(TOKEN_AT.into(), Kind::IDENTIFIERS);

            this.next_while_trivia();
            this.node_expression_single(until);
         })
         .call();
   }

   fn node_identifier(&mut self, until: EnumSet<Kind>) {
      if self.peek() == Some(TOKEN_QUOTED_IDENTIFIER_START) {
         self.node_delimited();
      } else {
         self
            .node(NODE_IDENTIFIER)
            .with(|this| this.next_expect(Kind::IDENTIFIERS, until))
            .call();
      }
   }

   fn node_delimited(&mut self) -> Option<&str> {
      let start_of_delimited = self.checkpoint();

      let (node, end) = self
         .next()
         .into_node_and_closing()
         .expect("node_delimited must be called right before a starting delimiter");

      let mut end_delimiter = None;

      self
         .node(node)
         .from(start_of_delimited)
         .with(|this| {
            loop {
               match this.peek() {
                  Some(TOKEN_CONTENT) => {
                     this.next_direct();
                  },

                  Some(TOKEN_INTERPOLATION_START) => {
                     this.node_interpolation();
                  },

                  Some(other) if other == end => {
                     end_delimiter = this.tokens.peek().map(|&(_, slice)| slice);
                     this.next_direct();
                     break;
                  },

                  Some(_) => {
                     // Sometimes recoverably parsing interpolation leaves us unwanted tokens. It
                     // is not worth it trying to node it correctly without a big rewrite, so
                     // just consume them.
                     this.next_direct();
                  },

                  None => {
                     this.reports.push(
                        unexpected()
                           .expected(TOKEN_CONTENT | end)
                           .span(Span::empty(this.offset))
                           .call(),
                     );
                     break;
                  },
               }
            }
         })
         .call();

      end_delimiter
   }

   fn node_interpolation(&mut self) {
      self
         .node(NODE_INTERPOLATION)
         .with(|this| {
            this.next_expect(TOKEN_INTERPOLATION_START.into(), EnumSet::empty());

            this.node_expression(TOKEN_INTERPOLATION_END.into());

            this.next_expect(TOKEN_INTERPOLATION_END.into(), EnumSet::empty());
         })
         .call();
   }

   fn node_integer(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_INTEGER)
         .with(|this| this.next_expect(TOKEN_INTEGER.into(), until))
         .call();
   }

   fn node_float(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_FLOAT)
         .with(|this| this.next_expect(TOKEN_FLOAT.into(), until))
         .call();
   }

   fn node_if(&mut self, until: EnumSet<Kind>) {
      let if_then_else_binding_power = node::InfixOperator::Same.binding_power().0 + 1;

      self
         .node(NODE_IF)
         .with(|this| {
            this.next_expect(
               TOKEN_KEYWORD_IF.into(),
               until | Kind::EXPRESSIONS | TOKEN_KEYWORD_THEN | TOKEN_KEYWORD_ELSE,
            );

            this.node_expression_binding_power(
               if_then_else_binding_power,
               until | Kind::EXPRESSIONS | TOKEN_KEYWORD_THEN | TOKEN_KEYWORD_ELSE,
            );

            this.next_expect(
               TOKEN_KEYWORD_THEN.into(),
               until | Kind::EXPRESSIONS | TOKEN_KEYWORD_ELSE,
            );

            this.node_expression_binding_power(
               if_then_else_binding_power,
               until | Kind::EXPRESSIONS | TOKEN_KEYWORD_ELSE,
            );

            this.next_expect(TOKEN_KEYWORD_ELSE.into(), until | Kind::EXPRESSIONS);

            this.node_expression_binding_power(if_then_else_binding_power, until);
         })
         .call();
   }

   fn node_expression_single(&mut self, until: EnumSet<Kind>) {
      let expected_at = self.checkpoint();

      match self.peek() {
         Some(TOKEN_PARENTHESIS_LEFT) => self.node_parenthesis(until),

         Some(TOKEN_BRACKET_LEFT) => self.node_list(until),

         Some(TOKEN_CURLYBRACE_LEFT) => self.node_attributes(until),

         Some(TOKEN_PATH_ROOT_TYPE_START | TOKEN_PATH_SUBPATH_START) => self.node_path(until),

         Some(TOKEN_STRING_START | TOKEN_RUNE_START) => {
            self.node_delimited();
         },

         Some(TOKEN_AT) => self.node_bind(until),

         Some(next) if Kind::IDENTIFIERS.contains(next) => self.node_identifier(until),

         Some(TOKEN_INTEGER) => self.node_integer(until),
         Some(TOKEN_FLOAT) => self.node_float(until),

         Some(TOKEN_KEYWORD_IF) => self.node_if(until),

         got => {
            // Consume until the next token is either the limit, an
            // expression token or an operator.
            let got_span = self.next_while(|kind| {
               !((until | Kind::EXPRESSIONS).contains(kind)
                  || node::PrefixOperator::try_from(kind).is_ok()
                  || node::InfixOperator::try_from(kind)
                     .is_ok_and(node::InfixOperator::is_token_owning)
                  || node::SuffixOperator::try_from(kind).is_ok())
            });

            self.node(NODE_ERROR).from(expected_at).with(|_| {}).call();

            self.reports.push(
               unexpected()
                  .maybe_got(got)
                  .expected(Kind::EXPRESSIONS)
                  .span(got_span)
                  .call(),
            );
         },
      }
   }

   fn node_expression_binding_power(&mut self, minimum_power: u16, until: EnumSet<Kind>) {
      let start_of_expression = self.checkpoint();

      if let Some(operator) = self
         .peek()
         .and_then(|kind| node::PrefixOperator::try_from(kind).ok())
      {
         let ((), right_power) = operator.binding_power();

         self
            .node(NODE_PREFIX_OPERATION)
            .with(|this| {
               this.next();
               this.node_expression_binding_power(right_power, until);
            })
            .call();
      } else {
         self.node_expression_single(until);
      }

      while let Some(operator) = self
         .peek()
         .and_then(|kind| node::InfixOperator::try_from(kind).ok())
      {
         let (left_power, right_power) = operator.binding_power();
         if left_power < minimum_power {
            break;
         }

         let operator_token = operator.is_token_owning().then(|| self.next());

         // Handle suffix-able infix operators. Not for purely suffix operators.
         if let Some(operator_token) = operator_token
            && node::SuffixOperator::try_from(operator_token).is_ok()
            && self
               .peek()
               .is_none_or(|kind| !Kind::EXPRESSIONS.contains(kind))
         {
            self
               .node(NODE_SUFFIX_OPERATION)
               .from(start_of_expression)
               .with(|_| {})
               .call();
         } else {
            self
               .node(NODE_INFIX_OPERATION)
               .from(start_of_expression)
               .with(|this| this.node_expression_binding_power(right_power, until))
               .call();
         }
      }
   }

   fn node_expression(&mut self, until: EnumSet<Kind>) {
      self.node_expression_binding_power(0, until);
   }
}

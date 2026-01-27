use std::{
   borrow::Cow,
   fmt::Write as _,
   sync::Arc,
};

use cab_util::into;
use cyn::{
   Result,
   ResultExt as _,
   bail,
};
use dup::Dupe;
use enumset::EnumSet;
use peekmore::{
   PeekMore as _,
   PeekMoreIterator as PeekMore,
};
use ranged::{
   IntoSize as _,
   Size,
   Span,
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
#[derive(Debug, Clone, Dupe, PartialEq, Eq)]
pub struct Parse {
   /// The [`node::Expression`].
   pub expression: node::Expression,

   /// The underlying node.
   pub node: red::Node,

   /// Issues reported during parsing.
   pub reports: Arc<[Report]>,
}

impl Parse {
   pub fn extractlnln(
      self,
      writer: &mut impl Write,
      location: &impl Display,
      source: &report::PositionStr<'_>,
   ) -> Result<node::Expression> {
      let mut fail: usize = 0;

      for report in &*self.reports {
         if let report::Severity::Error | report::Severity::Bug = report.severity {
            fail += 1;
         }

         writer
            .write_report(report, location, source)
            .chain_err("failed to write report")?;

         write!(writer, "\n\n").chain_err("failed to write report")?;
      }

      if fail > 0 {
         bail!(
            "parsing failed due to {fail} previous error{s}",
            s = if fail == 1 { "" } else { "s" },
         );
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

      noder.node(NODE_PARSE_ROOT).with(|this| {
         this.node_expression(EnumSet::empty());
         this.next_expect(EnumSet::empty(), EnumSet::empty());
      });

      let (green_node, _) = noder.builder.finish();

      let node = red::Node::new_root_with_resolver(green_node, self.cache.interner().dupe());

      let expression = node::ExpressionRef::try_from(
         node
            .first_child()
            .expect("noder output must contain a single parse root node"),
      )
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
         reports: Arc::from(noder.reports),
      }
   }
}

#[bon::builder(finish_fn(name = "expected"))]
fn unexpected(
   #[builder(start_fn)] span: Span,
   #[builder(finish_fn)] mut expected: EnumSet<Kind>,
   got: Option<Kind>,
) -> Report {
   let report = if expected.is_empty() {
      Report::error("expected end of file")
   } else {
      let mut title = String::from("expected ");

      if expected.is_superset(Kind::EXPRESSIONS) {
         expected.remove_all(Kind::EXPRESSIONS);

         let separator = match expected.len() {
            0 => "",
            1 => " or ",
            2.. => ", ",
         };

         let _ = write!(title, "an expression{separator}");
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

         let _ = write!(title, "{item}{separator}");
      }

      Report::error(title)
   };

   report.primary(span, match got {
      Some(kind) => Cow::Owned(format!("got {kind}")),
      None => Cow::Borrowed("reached end of file"),
   })
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

   #[builder(finish_fn(name = "with"))]
   #[inline]
   fn node<T>(
      &mut self,
      #[builder(start_fn)] kind: Kind,
      #[builder(finish_fn)] with: impl FnOnce(&mut Self) -> T,
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
            self
               .reports
               .push(unexpected(Span::empty(self.offset)).expected(EnumSet::empty()));

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

   fn next_expect(
      &mut self,
      expected: impl Into<EnumSet<Kind>>,
      until: EnumSet<Kind>,
   ) -> Option<Kind> {
      into!(expected);

      let expected_at = self.checkpoint();

      match self.peek() {
         None if expected.is_empty() => None,
         Some(next) if expected.contains(next) => Some(self.next()),

         got => {
            let got_span = self.next_while(|next| !(until | expected).contains(next));

            self.node(NODE_ERROR).from(expected_at).with(|_| {});

            self
               .reports
               .push(unexpected(got_span).maybe_got(got).expected(expected));

            let next = self.peek()?;

            expected.contains(next).then(|| self.next())
         },
      }
   }

   fn node_parenthesis(&mut self, until: EnumSet<Kind>) {
      self.node(NODE_PARENTHESIS).with(|this| {
         this.next_expect(
            TOKEN_PARENTHESIS_LEFT,
            until | Kind::EXPRESSIONS | TOKEN_PARENTHESIS_RIGHT,
         );

         if this
            .peek()
            .is_some_and(|kind| kind != TOKEN_PARENTHESIS_RIGHT)
         {
            this.node_expression(until | TOKEN_PARENTHESIS_RIGHT);
         }

         this.next_if(TOKEN_PARENTHESIS_RIGHT);
      });
   }

   fn node_list(&mut self, until: EnumSet<Kind>) {
      self.node(NODE_LIST).with(|this| {
         this.next_expect(
            TOKEN_BRACKET_LEFT,
            until | Kind::EXPRESSIONS | TOKEN_BRACKET_RIGHT,
         );

         if this.peek().is_some_and(|kind| kind != TOKEN_BRACKET_RIGHT) {
            this.node_expression(until | TOKEN_BRACKET_RIGHT);
         }

         this.next_if(TOKEN_BRACKET_RIGHT);
      });
   }

   fn node_attributes(&mut self, until: EnumSet<Kind>) {
      self.node(NODE_ATTRIBUTES).with(|this| {
         this.next_expect(
            TOKEN_CURLYBRACE_LEFT,
            until | Kind::EXPRESSIONS | TOKEN_CURLYBRACE_RIGHT,
         );

         if this
            .peek()
            .is_some_and(|kind| kind != TOKEN_CURLYBRACE_RIGHT)
         {
            this.node_expression(until | TOKEN_CURLYBRACE_RIGHT);
         }

         this.next_if(TOKEN_CURLYBRACE_RIGHT);
      });
   }

   fn node_bind(&mut self, until: EnumSet<Kind>) {
      self.node(NODE_BIND).with(|this| {
         this.next_expect(TOKEN_AT, Kind::IDENTIFIERS);

         this.next_while_trivia();
         this.node_expression_single(until);
      });
   }

   fn node_identifier(&mut self, until: EnumSet<Kind>) {
      if self.peek() == Some(TOKEN_QUOTED_IDENTIFIER_START) {
         self.node_delimited();
      } else {
         self
            .node(NODE_IDENTIFIER)
            .with(|this| this.next_expect(Kind::IDENTIFIERS, until));
      }
   }

   fn node_delimited(&mut self) {
      let start_of_delimited = self.checkpoint();

      let (node, end) = self
         .next()
         .into_node_and_closing()
         .expect("node_delimited must be called right before a starting delimiter");

      self.node(node).from(start_of_delimited).with(|this| {
         loop {
            match this.peek() {
               Some(TOKEN_CONTENT) => {
                  this.next_direct();
               },

               Some(TOKEN_INTERPOLATION_START) => {
                  this.node_interpolation();
               },

               Some(other) if other == end => {
                  this.next_direct();
                  break;
               },

               // Break here and validate later. We know `"foo` is a string.
               Some(_) | None => break,
            }
         }
      });
   }

   fn node_interpolation(&mut self) {
      self.node(NODE_INTERPOLATION).with(|this| {
         this.next_expect(TOKEN_INTERPOLATION_START, EnumSet::empty());

         this.node_expression(EnumSet::new() | TOKEN_INTERPOLATION_END);

         this.next_expect(TOKEN_INTERPOLATION_END, EnumSet::empty());
      });
   }

   fn node_integer(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_INTEGER)
         .with(|this| this.next_expect(TOKEN_INTEGER, until));
   }

   fn node_float(&mut self, until: EnumSet<Kind>) {
      self
         .node(NODE_FLOAT)
         .with(|this| this.next_expect(TOKEN_FLOAT, until));
   }

   fn node_if(&mut self, until: EnumSet<Kind>) {
      let if_then_else_binding_power = node::InfixOperator::Same.binding_power().0 + 1;

      self.node(NODE_IF).with(|this| {
         this.next_expect(
            TOKEN_KEYWORD_IF,
            until | Kind::EXPRESSIONS | TOKEN_KEYWORD_THEN | TOKEN_KEYWORD_ELSE,
         );

         this.node_expression_binding_power(
            if_then_else_binding_power,
            until | Kind::EXPRESSIONS | TOKEN_KEYWORD_THEN | TOKEN_KEYWORD_ELSE,
         );

         this.next_expect(
            TOKEN_KEYWORD_THEN,
            until | Kind::EXPRESSIONS | TOKEN_KEYWORD_ELSE,
         );

         this.node_expression_binding_power(
            if_then_else_binding_power,
            until | Kind::EXPRESSIONS | TOKEN_KEYWORD_ELSE,
         );

         this.next_expect(TOKEN_KEYWORD_ELSE, until | Kind::EXPRESSIONS);

         this.node_expression_binding_power(if_then_else_binding_power, until);
      });
   }

   #[stacksafe::stacksafe]
   fn node_expression_single(&mut self, until: EnumSet<Kind>) {
      let expected_at = self.checkpoint();

      match self.peek() {
         Some(TOKEN_PARENTHESIS_LEFT) => self.node_parenthesis(until),

         Some(TOKEN_BRACKET_LEFT) => self.node_list(until),

         Some(TOKEN_CURLYBRACE_LEFT) => self.node_attributes(until),

         Some(TOKEN_INTEGER) => self.node_integer(until),
         Some(TOKEN_FLOAT) => self.node_float(until),

         Some(TOKEN_KEYWORD_IF) => self.node_if(until),

         Some(TOKEN_PATH_START) => self.node_delimited(),

         Some(TOKEN_AT) => self.node_bind(until),
         Some(next) if Kind::IDENTIFIERS.contains(next) => self.node_identifier(until),

         Some(TOKEN_STRING_START | TOKEN_CHAR_START) => self.node_delimited(),

         // The rest are errors.
         Some(kind) if Kind::EXPRESSIONS.contains(kind) => {
            let start = self.offset;
            self.node(NODE_ERROR).with(Self::next);

            self.reports.push(
               unexpected(Span::new(start, self.offset))
                  .got(kind)
                  .expected(Kind::EXPRESSIONS),
            );
         },

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

            self.node(NODE_ERROR).from(expected_at).with(|_| {});

            self.reports.push(
               unexpected(got_span)
                  .maybe_got(got)
                  .expected(Kind::EXPRESSIONS),
            );
         },
      }
   }

   #[stacksafe::stacksafe]
   fn node_expression_binding_power(&mut self, minimum_power: u16, until: EnumSet<Kind>) {
      let mut noded = false;

      let start_of_expression = self.checkpoint();

      if let Some(operator) = self
         .peek()
         .and_then(|kind| node::PrefixOperator::try_from(kind).ok())
      {
         let ((), right_power) = operator.binding_power();

         self.node(NODE_PREFIX_OPERATION).with(|this| {
            noded = true;

            this.next();

            if this
               .peek()
               .is_some_and(|kind| Kind::EXPRESSIONS.contains(kind))
            {
               this.node_expression_binding_power(right_power, until);
            }
         });
      } else if self
         .peek()
         .is_some_and(|kind| Kind::EXPRESSIONS.contains(kind))
      {
         noded = true;
         self.node_expression_single(until);
      }

      loop {
         match self.peek() {
            Some(kind) if let Ok(operator) = node::InfixOperator::try_from(kind) => {
               let (left_power, right_power) = operator.binding_power();
               if left_power < minimum_power {
                  break;
               }

               self
                  .node(NODE_INFIX_OPERATION)
                  .from(start_of_expression)
                  .with(|this| {
                     noded = true;

                     if operator.is_token_owning() {
                        this.next();
                     }

                     if this
                        .peek()
                        .is_some_and(|kind| Kind::EXPRESSIONS.contains(kind))
                     {
                        this.node_expression_binding_power(right_power, until);
                     }
                  });
            },

            Some(kind) if let Ok(operator) = node::SuffixOperator::try_from(kind) => {
               let (left_power, ()) = operator.binding_power();
               if left_power < minimum_power {
                  break;
               }

               self
                  .node(NODE_SUFFIX_OPERATION)
                  .from(start_of_expression)
                  .with(|this| {
                     noded = true;

                     this.next();
                  });
            },

            _ => break,
         }
      }

      if !noded {
         let got_span = self.next_while(|kind| !until.contains(kind));

         self.node(NODE_ERROR).from(start_of_expression).with(|_| {});

         self
            .reports
            .push(unexpected(got_span).expected(Kind::EXPRESSIONS));
      }
   }

   fn node_expression(&mut self, until: EnumSet<Kind>) {
      self.node_expression_binding_power(0, until);
   }
}

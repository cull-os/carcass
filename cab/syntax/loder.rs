use std::{
   borrow::Cow,
   cell,
   sync::Arc,
};

use cab_util::{
   force,
   lazy,
   read,
   ready,
};
use cyn::{
   Result,
   ResultExt as _,
   bail,
};
use ranged::{
   IntoSpan as _,
   Span,
   Spanned,
   SpannedExt as _,
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
   lode::{
      self,
      ResolvedExt as _,
   },
   node::{
      self,
      Segmented as _,
   },
   token,
};

const CURRY_LEFT: &str = "left";
const CURRY_RIGHT: &str = "right";

#[derive(Debug, Clone)]
pub struct Lower {
   expression: lode::ExpressionId,
   arena:      slotmap::SlotMap<lode::ExpressionId, lode::Expression>,

   pub reports: Arc<[Report]>,
}

impl Lower {
   #[must_use]
   pub fn expression(&self) -> lode::Resolved<'_, &lode::Expression> {
      self
         .arena
         .get(self.expression)
         .expect("lode expression must be in arena")
         .resolved(&self.arena)
   }

   pub fn extractlnln(
      self,
      writer: &mut impl Write,
      location: &impl Display,
      source: &report::PositionStr<'_>,
   ) -> Result<Self> {
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
            "lowering failed due to {fail} previous error{s}",
            s = if fail == 1 { "" } else { "s" },
         );
      }

      Ok(self)
   }
}

pub struct LowerOracle {
   _reserved: (),
}

impl LowerOracle {
   #[must_use]
   pub fn new() -> Self {
      Self { _reserved: () }
   }

   #[must_use]
   #[expect(clippy::unused_self)]
   pub fn lower(&self, expression: node::ExpressionRef<'_>) -> Lower {
      let mut loder = Loder::new();
      let expression = loder.lode(expression);

      Lower {
         expression,
         arena: loder.arena,
         reports: Arc::from(loder.reports),
      }
   }
}

struct Loder {
   arena:   slotmap::SlotMap<lode::ExpressionId, lode::Expression>,
   reports: Vec<Report>,
}

#[bon::bon]
impl Loder {
   fn new() -> Self {
      Self {
         arena:   slotmap::SlotMap::with_key(),
         reports: Vec::new(),
      }
   }

   fn insert(&mut self, expression: Spanned<impl Into<lode::ExpressionRaw>>) -> lode::ExpressionId {
      let span = expression.span();
      self.arena.insert(expression.value.into().spanned(span))
   }

   fn throw(&mut self, message: Spanned<impl Into<Cow<'static, str>>>) -> lode::ExpressionRaw {
      let message = message.map(Into::into);
      let span = message.span();

      let throw = self.refence("throw".spanned(span));

      lode::Call {
         function: throw,
         argument: self.insert(lode::SString(lode::Segments::plain(message)).spanned(span)),
      }
      .into()
   }

   fn refence(&mut self, identifier: Spanned<&'static str>) -> lode::ExpressionId {
      self.insert(
         lode::Identifier(lode::Segments::plain(
            identifier.value.spanned(identifier.span()),
         ))
         .spanned(identifier.span()),
      )
   }

   fn bind(&mut self, identifier: Spanned<&'static str>) -> lode::ExpressionId {
      self.insert(
         lode::Bind(lode::Segments::plain(
            identifier.value.spanned(identifier.span()),
         ))
         .spanned(identifier.span()),
      )
   }

   #[builder(finish_fn(name = "report"))]
   fn lode_segments(
      &mut self,
      #[builder(start_fn)] segmented: &impl node::Segmented,
      #[builder(finish_fn)] mut report: cell::LazyCell<Report, fn() -> Report>,
      closing: Option<(Kind, &'static str)>,
   ) -> Option<lode::Segments> {
      #[expect(
         clippy::collapsible_if,
         reason = "bon::builder currently errors on let-chain form for this method"
      )]
      if let Some((end, type_)) = closing {
         if segmented
            .children_with_tokens()
            .last()
            .is_none_or(|token| token.kind() != end)
         {
            let start = segmented
               .children_with_tokens()
               .next()
               .expect("delimited must have tokens");

            self.reports.push(
               Report::error(format!("unclosed {type_}"))
                  .secondary(start.span(), format!("{type_} starts here"))
                  .primary(
                     Span::empty(segmented.span().end),
                     format!("expected {end} here"),
                  ),
            );

            return None;
         }
      }

      let segments = segmented.segments();

      for straight in &segments.straights {
         let node::Straight::Line { span, text, .. } = *straight else {
            continue;
         };

         let Err(invalids) = token::unescape_string(text) else {
            continue;
         };

         for invalid in invalids {
            force!(report).push_primary(invalid.offset(span.start), "invalid escape");
         }
      }

      if let Err(indents) = segments.indent() {
         force!(report).push_primary(
            segments.span,
            format!(
               "cannot mix different kinds of space in indents: {indents}",
               indents = indents
                  .into_iter()
                  .map(|c| {
                     match token::escape(c)
                        .is_first(true)
                        .delimiter(('\'', "\\'"))
                        .call()
                     {
                        Some(escaped) => escaped.to_owned(),
                        None => format!("'{c}'"),
                     }
                  })
                  .intersperse(", ".to_owned())
                  .collect::<String>(),
            ),
         );
      }

      if segments.is_multiline {
         for span in [segments.line_span_first, segments.line_span_last]
            .into_iter()
            .flatten()
         {
            force!(report).push_primary(span, "first and last lines must be empty");
         }
      }

      if let Some(report) = read!(report) {
         self.reports.push(report);
         return None;
      }

      Some(lode::Segments(
         segments
            .into_iter()
            .map(|segment| {
               match segment {
                  node::Segment::Content { span, content } => {
                     lode::Segment::Content(Cow::<'static, str>::Owned(content).spanned(span))
                  },

                  node::Segment::Interpolation(interpolation) => {
                     lode::Segment::Interpolation(self.lode(interpolation.expression()))
                  },
               }
            })
            .collect::<Vec<_>>(),
      ))
   }

   fn lode_parenthesis(&mut self, parenthesis: &node::Parenthesis) -> lode::ExpressionRaw {
      let expression = match parenthesis.expression() {
         Some(expression) => self.lode(expression),

         None => {
            self.reports.push(
               Report::error("parenthesis without inner expression").primary(
                  Span::empty(parenthesis.token_parenthesis_left().span().end),
                  "expected an expression here",
               ),
            );

            let throw =
               self.throw("parenthesis without inner expression".spanned(parenthesis.span()));

            self.insert(throw.spanned(parenthesis.span()))
         },
      };

      if parenthesis.token_parenthesis_right().is_none() {
         self.reports.push(
            Report::error("unclosed parenthesis")
               .primary(Span::empty(parenthesis.span().end), "expected ')' here")
               .secondary(
                  parenthesis.token_parenthesis_left().span(),
                  "unclosed '(' here",
               ),
         );
      }

      lode::Parenthesis { expression }.into()
   }

   // TODO: Refactor into x:y:z:[]
   fn lode_list(&mut self, list: &node::List) -> lode::ExpressionRaw {
      if let Some(node::ExpressionRef::InfixOperation(operation)) = list.expression()
         && operation.operator() == node::InfixOperator::Sequence
      {
         self.reports.push(
            Report::error("inner expression of list cannot be sequence")
               .primary(operation.span(), "consider parenthesizing this"),
         );
      }

      if list.token_bracket_right().is_none() {
         self.reports.push(
            Report::error("unclosed list")
               .primary(Span::empty(list.span().end), "expected ']' here")
               .secondary(list.token_bracket_left().span(), "unclosed '[' here"),
         );
      }

      lode::List {
         items: list.items().map(|item| self.lode(item)).collect(),
      }
      .into()
   }

   fn lode_attributes(&mut self, attributes: &node::Attributes) -> lode::ExpressionRaw {
      if attributes.token_curlybrace_right().is_none() {
         self.reports.push(
            Report::error("unclosed attributes")
               .primary(Span::empty(attributes.span().end), "expected '}' here")
               .secondary(
                  attributes.token_curlybrace_left().span(),
                  "unclosed '{' here",
               ),
         );
      }

      lode::Attributes {
         expression: attributes
            .expression()
            .map(|expression| self.lode(expression)),
      }
      .into()
   }

   fn lode_prefix_operation(&mut self, operation: &node::PrefixOperation) -> lode::ExpressionRaw {
      let (right, right_is_missing) = match operation.right() {
         Some(right) => (self.lode(right), false),
         None => (self.refence(CURRY_RIGHT.spanned(operation.span())), true),
      };

      let expression = lode::Select {
         scope:      right,
         expression: self.refence(
            match operation.operator() {
               node::PrefixOperator::Swwallation => "+",
               node::PrefixOperator::Negation => "-",
               node::PrefixOperator::Not => "!",
            }
            .spanned(operation.operator_token().span()),
         ),
      }
      .into();

      match (right_is_missing,) {
         (false,) => expression,

         (true,) => {
            let expression = self.insert(expression.spanned(operation.span()));

            lode::Lambda {
               argument: self.bind(CURRY_RIGHT.spanned(operation.span())),
               expression,
            }
            .into()
         },
      }
   }

   fn lode_infix_operation(&mut self, operation: &node::InfixOperation) -> lode::ExpressionRaw {
      let operator = operation.operator();

      if let node::InfixOperator::Call | node::InfixOperator::Pipe = operator {
         for expression in [operation.left(), operation.right()].into_iter().flatten() {
            if let node::ExpressionRef::InfixOperation(child_operation) = expression
               && let child_operator @ (node::InfixOperator::Call | node::InfixOperator::Pipe) =
                  child_operation.operator()
               && child_operator != operator
            {
               self.reports.push(
                  Report::error("call and pipe operators do not associate")
                     .secondary(operation.span(), "this")
                     .primary(child_operation.span(), "does not associate with this"),
               );
            }
         }
      }

      let (left, left_is_missing) = match operation.left() {
         Some(left) => (self.lode(left), false),
         None => (self.refence(CURRY_LEFT.spanned(operation.span())), true),
      };

      let (right, right_is_missing) = match operation.right() {
         Some(right) => (self.lode(right), false),
         None => (self.refence(CURRY_RIGHT.spanned(operation.span())), true),
      };

      let expression = match operator {
         node::InfixOperator::Same => lode::Same { left, right }.into(),
         node::InfixOperator::Sequence => lode::Sequence { left, right }.into(),

         node::InfixOperator::ImplicitCall | node::InfixOperator::Call => {
            lode::Call {
               function: left,
               argument: right,
            }
            .into()
         },
         node::InfixOperator::Pipe => {
            lode::Call {
               function: right,
               argument: left,
            }
            .into()
         },

         node::InfixOperator::Construct => {
            lode::Construct {
               head: left,
               tail: right,
            }
            .into()
         },

         node::InfixOperator::Select => {
            lode::Select {
               scope:      left,
               expression: right,
            }
            .into()
         },

         node::InfixOperator::Equal => lode::Equal { left, right }.into(),
         node::InfixOperator::NotEqual => {
            let equal = self.insert(lode::Equal { left, right }.spanned(operation.span()));

            lode::Select {
               scope:      equal,
               expression: self.refence(
                  "!".spanned(
                     operation
                        .operator_token()
                        .expect("operator token must exist")
                        .span(),
                  ),
               ),
            }
            .into()
         },

         node::InfixOperator::And => lode::And { left, right }.into(),
         node::InfixOperator::Or => lode::Or { left, right }.into(),
         node::InfixOperator::Implication => {
            let attribute = self.refence(
               "!".spanned(
                  operation
                     .operator_token()
                     .expect("operator token must exist")
                     .span(),
               ),
            );

            let left = self.insert(
               lode::Select {
                  scope:      left,
                  expression: attribute,
               }
               .spanned(operation.span()),
            );

            lode::Or { left, right }.into()
         },

         node::InfixOperator::All => lode::All { left, right }.into(),
         node::InfixOperator::Any => lode::Any { left, right }.into(),

         node::InfixOperator::Lambda => {
            lode::Lambda {
               argument:   left,
               expression: right,
            }
            .into()
         },

         operator @ (node::InfixOperator::Concat
         | node::InfixOperator::Update
         | node::InfixOperator::LessOrEqual
         | node::InfixOperator::Less
         | node::InfixOperator::MoreOrEqual
         | node::InfixOperator::More
         | node::InfixOperator::Addition
         | node::InfixOperator::Subtraction
         | node::InfixOperator::Multiplication
         | node::InfixOperator::Power
         | node::InfixOperator::Division) => {
            let attribute = self.refence(
               match operator {
                  node::InfixOperator::Concat => "++",
                  node::InfixOperator::Update => "//",
                  node::InfixOperator::LessOrEqual => "<=",
                  node::InfixOperator::Less => "<",
                  node::InfixOperator::MoreOrEqual => ">=",
                  node::InfixOperator::More => ">",
                  node::InfixOperator::Addition => "+",
                  node::InfixOperator::Subtraction => "-",
                  node::InfixOperator::Multiplication => "*",
                  node::InfixOperator::Power => "^",
                  node::InfixOperator::Division => "/",

                  _ => unreachable!(),
               }
               .spanned(
                  operation
                     .operator_token()
                     .expect("operator token must exist")
                     .span(),
               ),
            );

            let method = self.insert(
               lode::Select {
                  scope:      left,
                  expression: attribute,
               }
               .spanned(operation.span()),
            );

            lode::Call {
               function: method,
               argument: right,
            }
            .into()
         },
      };

      match (left_is_missing, right_is_missing) {
         (false, false) => expression,

         (true, false) => {
            let expression = self.insert(expression.spanned(operation.span()));

            let left = self.bind(CURRY_LEFT.spanned(operation.span()));

            lode::Lambda {
               argument: left,
               expression,
            }
            .into()
         },

         (false, true) => {
            let expression = self.insert(expression.spanned(operation.span()));

            let right = self.bind(CURRY_RIGHT.spanned(operation.span()));

            lode::Lambda {
               argument: right,
               expression,
            }
            .into()
         },

         (true, true) => {
            let expression = self.insert(expression.spanned(operation.span()));

            let left = self.bind(CURRY_LEFT.spanned(operation.span()));
            let right = self.bind(CURRY_RIGHT.spanned(operation.span()));

            lode::Lambda {
               argument:   left,
               expression: {
                  self.insert(
                     lode::Lambda {
                        argument: right,
                        expression,
                     }
                     .spanned(operation.span()),
                  )
               },
            }
            .into()
         },
      }
   }

   #[expect(unreachable_code, unused_variables)]
   fn lode_suffix_operation(&mut self, operation: &node::SuffixOperation) -> lode::ExpressionRaw {
      let (left, left_is_missing) = match operation.left() {
         Some(left) => (self.lode(left), false),
         None => (self.refence(CURRY_LEFT.spanned(operation.span())), true),
      };

      let expression = match operation.operator() {};

      match (left_is_missing,) {
         (false,) => expression,

         (true,) => {
            let expression = self.insert(expression.spanned(operation.span()));

            lode::Lambda {
               argument: self.bind(CURRY_LEFT.spanned(operation.span())),
               expression,
            }
            .into()
         },
      }
   }

   fn lode_path(&mut self, path: &node::Path) -> lode::ExpressionRaw {
      let Some(segments) = self
         .lode_segments(path)
         .report(lazy!(Report::error("invalid path")))
      else {
         return self.throw("invalid path".spanned(path.span()));
      };

      // Only assert if segment validation was clean. For example,
      // `/etc/ssl\<newline-here>` gets parsed as multiline and should already
      // have emitted validation labels.
      assert!(!path.segments().is_multiline);

      lode::Path(segments).into()
   }

   fn lode_bind(&mut self, bind: &node::Bind) -> lode::ExpressionRaw {
      let expression = bind.expression();

      let node::ExpressionRef::Identifier(identifier) = expression else {
         if expression.kind() != NODE_ERROR {
            self.reports.push(Report::error("invalid bind").primary(
               expression.span(),
               format!(
                  "expected an identifier, got {kind}",
                  kind = expression.kind(),
               ),
            ));
         }

         return self.throw("invalid bind".spanned(bind.span()));
      };

      self.lode_identifier(identifier).is_bind(true).call()
   }

   #[builder]
   fn lode_identifier(
      &mut self,
      #[builder(start_fn)] identifier: &node::Identifier,
      #[builder(default)] is_bind: bool,
   ) -> lode::ExpressionRaw {
      let segments = match identifier.value() {
         node::IdentifierValueRef::Plain(identifier) => {
            lode::Segments::plain(identifier.text().to_owned().spanned(identifier.span()))
         },

         node::IdentifierValueRef::Quoted(quoted) => {
            let Some(segments) = self
               .lode_segments(quoted)
               .closing((TOKEN_QUOTED_IDENTIFIER_END, "quoted identifier"))
               .report(lazy!(Report::error("invalid quoted identifier")))
            else {
               return self.throw("invalid identifier".spanned(identifier.span()));
            };

            if quoted.segments().is_multiline {
               self.reports.push(
                  Report::error("invalid quoted identifier")
                     .primary(quoted.span(), "here")
                     .tip("quoted identifiers cannot contain newlines"),
               );
            }

            segments
         },
      };

      if is_bind {
         lode::Bind(segments).into()
      } else {
         lode::Identifier(segments).into()
      }
   }

   fn lode_string(&mut self, string: &node::SString) -> lode::ExpressionRaw {
      let Some(segments) = self
         .lode_segments(string)
         .closing((TOKEN_STRING_END, "string"))
         .report(lazy!(Report::error("invalid string")))
      else {
         return self.throw("invalid string".spanned(string.span()));
      };

      lode::SString(segments).into()
   }

   fn lode_char(&mut self, char: &node::Char) -> lode::ExpressionRaw {
      let Some(_) = self
         .lode_segments(char)
         .closing((TOKEN_CHAR_END, "char"))
         .report(lazy!(Report::error("invalid char")))
      else {
         return self.throw("invalid char".spanned(char.span()));
      };

      let segments = char.segments();
      let mut report = lazy!(Report::error("invalid char"));

      if segments.is_multiline {
         force!(report).push_primary(char.span(), "chars cannot cannot contain newlines");
      }

      if !ready!(report) {
         let mut got: usize = 0;

         for segment in segments {
            match segment {
               node::Segment::Content { content, .. } => {
                  got += content.chars().count();
               },

               node::Segment::Interpolation(interpolation) => {
                  force!(report)
                     .push_primary(interpolation.span(), "chars cannot contain interpolation");
               },
            }
         }

         match got {
            0 => force!(report).push_primary(char.span(), "empty char"),
            1 => {},
            2.. => force!(report).push_primary(char.span(), "too long"),
         }
      }

      if let Some(report) = read!(report) {
         self.reports.push(report);
         return self.throw("invalid char".spanned(char.span()));
      }

      char
         .value()
         .expect("char was validated and has first character")
         .into()
   }

   fn lode_integer(&mut self, integer: &node::Integer) -> lode::ExpressionRaw {
      let Ok(value) = integer.token_integer().value() else {
         self.reports.push(
            Report::error("invalid integer").primary(integer.span(), "why do you even need this?"),
         );

         return self.throw("invalid integer".spanned(integer.span()));
      };

      value.into()
   }

   fn lode_float(&mut self, float: &node::Float) -> lode::ExpressionRaw {
      let Ok(value) = float.token_float().value() else {
         self
            .reports
            .push(Report::error("invalid float").primary(float.span(), "usecase?"));

         return self.throw("invalid float".spanned(float.span()));
      };

      value.into()
   }

   fn lode_if(&mut self, if_: &node::If) -> lode::ExpressionRaw {
      lode::If {
         condition:   self.lode(if_.condition()),
         consequence: self.lode(if_.consequence()),
         alternative: self.lode(if_.alternative()),
      }
      .into()
   }

   fn lode(&mut self, expression: node::ExpressionRef<'_>) -> lode::ExpressionId {
      let expression = match expression {
         node::ExpressionRef::Error(error) => self.throw("syntax error".spanned(error.span())),

         node::ExpressionRef::Parenthesis(parenthesis) => self.lode_parenthesis(parenthesis),
         node::ExpressionRef::List(list) => self.lode_list(list),
         node::ExpressionRef::Attributes(attributes) => self.lode_attributes(attributes),

         node::ExpressionRef::PrefixOperation(operation) => self.lode_prefix_operation(operation),
         node::ExpressionRef::InfixOperation(operation) => self.lode_infix_operation(operation),
         node::ExpressionRef::SuffixOperation(operation) => self.lode_suffix_operation(operation),

         node::ExpressionRef::Path(path) => self.lode_path(path),

         node::ExpressionRef::Bind(bind) => self.lode_bind(bind),

         node::ExpressionRef::Identifier(identifier) => self.lode_identifier(identifier).call(),

         node::ExpressionRef::SString(string) => self.lode_string(string),

         node::ExpressionRef::Char(char) => self.lode_char(char),

         node::ExpressionRef::Integer(integer) => self.lode_integer(integer),

         node::ExpressionRef::Float(float) => self.lode_float(float),

         node::ExpressionRef::If(if_) => self.lode_if(if_),
      }
      .spanned(expression.span());

      self.insert(expression)
   }
}

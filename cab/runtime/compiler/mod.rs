use std::{
   ops,
   sync::Arc,
};

use cab_syntax::{
   Segment,
   Segmented as _,
   node,
};
use cab_util::{
   into,
   unwrap,
};
use cyn::{
   Result,
   ResultExt as _,
   bail,
};
use dup::Dupe as _;
use ranged::{
   IntoSpan as _,
   Span,
};
use smallvec::SmallVec;
use ust::{
   Display,
   Write,
   report::{
      self,
      Report,
   },
};

use crate::{
   Code,
   Operation,
   Value,
   value,
};

mod scope;
use scope::{
   LocalName,
   Scope,
};

const EXPECT_CODE: &str = "emitter must have at least one code at all times";
// const EXPECT_SCOPE: &str = "emitter must have at least one scope at all
// times";
const EXPECT_VALID: &str = "syntax must be valid";
const EXPECT_HANDLED: &str = "case was handled";

pub struct Compile {
   pub code:    Code,
   pub reports: Vec<Report>,
}

impl Compile {
   pub fn extractlnln(
      self,
      writer: &mut impl Write,
      location: &impl Display,
      source: &report::PositionStr<'_>,
   ) -> Result<Code> {
      let mut fail = 0;

      for report in self.reports {
         fail += usize::from(report.severity >= report::Severity::Error);

         writer
            .write_report(&report, location, source)
            .chain_err("failed to write report")?;

         write!(writer, "\n\n").chain_err("failed to write report")?;
      }

      if fail > 0 {
         bail!(
            "compilation failed due to {fail} previous error{s}",
            s = if fail == 1 { "" } else { "s" },
         );
      }

      Ok(self.code)
   }
}

pub struct CompileOracle {
   _reserved: (),
}

#[bon::bon]
impl CompileOracle {
   #[must_use]
   pub fn new() -> Self {
      Self { _reserved: () }
   }

   #[expect(clippy::unused_self)]
   #[builder(finish_fn(name = "path"))]
   #[must_use]
   pub fn compile(
      &self,
      #[builder(start_fn)] expression: node::ExpressionRef<'_>,
      #[builder(finish_fn)] path: value::Path,
   ) -> Compile {
      let mut emitter = Emitter::new(path);

      emitter.emit_scope(expression.span(), |this| {
         this.emit_force(expression);
      });

      emitter
         .reports
         .sort_by_key(|report| report.labels.iter().map(|label| label.span.start).min());

      Compile {
         code: emitter.codes.pop().expect(EXPECT_CODE),

         reports: emitter.reports,
      }
   }
}

struct Emitter<'a> {
   codes: Vec<Code>,

   scopes:  Vec<Scope<'a>>,
   reports: Vec<Report>,
}

impl ops::Deref for Emitter<'_> {
   type Target = Code;

   fn deref(&self) -> &Self::Target {
      self.codes.last().expect(EXPECT_CODE)
   }
}

impl ops::DerefMut for Emitter<'_> {
   fn deref_mut(&mut self) -> &mut Self::Target {
      self.codes.last_mut().expect(EXPECT_CODE)
   }
}

impl Emitter<'_> {
   fn new(path: value::Path) -> Self {
      Emitter {
         codes: vec![Code::new(path)],

         scopes:  vec![Scope::global()],
         reports: Vec::new(),
      }
   }

   // fn scope(&mut self) -> &mut Scope<'a> {
   //    self.scopes.last_mut().expect(EXPECT_SCOPE)
   // }
}

#[bon::bon]
impl<'a> Emitter<'a> {
   fn emit_push(&mut self, span: Span, value: impl Into<Value>) {
      into!(value);

      let index = self.value(value);

      self.push_operation(span, Operation::Push);
      self.push_u64(*index as _);
   }

   fn emit_scope(&mut self, span: Span, with: impl FnOnce(&mut Self)) {
      self.scopes.push(Scope::new());

      self.push_operation(span, Operation::ScopeStart);

      with(self);

      self.push_operation(span, Operation::ScopeEnd);

      // for local in self.scopes.pop().expect("scope was just pushed").finish()
      // {    self.reports.push(
      //       Report::warn(if let Ok(name) =
      // TryInto::<&str>::try_into(&local.name) {          format!("unused
      // bind '{name}'")       } else {
      //          "unused bind".to_owned()
      //       })
      //       .primary(local.span, "no usage")
      //       .tip("remove this or rename it to start with '_'"),
      //    );
      // }
   }

   fn emit_thunk_start(&mut self) {
      let path = self.path().dupe();
      self.codes.push(Code::new(path));
   }

   #[builder(finish_fn(name = "is_lambda"))]
   fn emit_thunk_end(
      &mut self,
      #[builder(start_fn)] span: Span,
      #[builder(finish_fn)] is_lambda: bool,
   ) {
      let code = self.codes.pop().expect(EXPECT_CODE);

      self.emit_push(
         span,
         if is_lambda {
            Value::Lambda(Arc::new(code))
         } else {
            Value::Suspend(Arc::new(code))
         },
      );
   }

   #[builder(finish_fn(name = "with"))]
   fn emit_thunk(
      &mut self,
      #[builder(start_fn)] span: Span,
      #[builder(finish_fn)] with: impl FnOnce(&mut Self),
      #[builder(default = true)] if_: bool,
      #[builder(default = false)] is_lambda: bool,
   ) {
      if !if_ {
         with(self);
         return;
      }

      self.emit_thunk_start();
      with(self);
      self.emit_thunk_end(span).is_lambda(is_lambda);
   }

   fn emit_parenthesis(&mut self, parenthesis: &'a node::Parenthesis) {
      self.emit_scope(parenthesis.span(), |this| {
         this.emit(parenthesis.expression().expect(EXPECT_VALID));
      });
   }

   fn emit_list(&mut self, list: &'a node::List) {
      let items = list.items().collect::<SmallVec<_, 8>>();
      let spans = items
         .iter()
         .map(|item| item.span())
         .collect::<SmallVec<_, 8>>();

      for item in items {
         self.emit_thunk_start();
         self.emit_scope(item.span(), |this| this.emit(item));
      }

      self.emit_push(list.span(), Value::Nil);

      for span in spans {
         self.push_operation(list.span(), Operation::Construct);
         self.emit_thunk_end(span).is_lambda(false);
      }
   }

   fn emit_attributes(&mut self, attributes: &'a node::Attributes) {
      match attributes.expression() {
         Some(expression) => {
            self.emit_thunk(attributes.span()).with(|this| {
               this.emit_scope(attributes.span(), |this| {
                  this.emit_force(expression);
                  let to_end = {
                     this.push_operation(expression.span(), Operation::JumpIfError);
                     this.push_u16(u16::default())
                  };
                  this.push_operation(expression.span(), Operation::ScopePush);
                  this.push_operation(expression.span(), Operation::Swap);
                  this.push_operation(expression.span(), Operation::Pop);
                  this.point_here(to_end);
               });
            });
         },

         None => {
            self.emit_push(attributes.span(), value::attributes::new! {});
         },
      }
   }

   fn emit_prefix_operation(&mut self, operation: &'a node::PrefixOperation) {
      let right = operation.right();

      self
         .emit_thunk(operation.span())
         .is_lambda(right.is_none())
         .with(|this| {
            if let Some(right) = right {
               this.emit_force(right);
            }

            this.push_operation(operation.span(), match operation.operator() {
               node::PrefixOperator::Swwallation => Operation::Swwallation,
               node::PrefixOperator::Negation => Operation::Negation,
               node::PrefixOperator::Not => Operation::Not,
            });
         });
   }

   fn emit_infix_operation(&mut self, operation: &'a node::InfixOperation) {
      let left = operation.left();
      let right = operation.right();

      self
         .emit_thunk(operation.span())
         .is_lambda(left.is_none() || right.is_none())
         .with(|this| {
            // TODO: Actually handle this.
            unwrap!(left, right);

            match operation.operator() {
               node::InfixOperator::Sequence => {
                  this.emit_force(left);
                  let to_end = {
                     this.push_operation(operation.span(), Operation::JumpIfError);
                     this.push_u16(u16::default())
                  };
                  this.push_operation(operation.span(), Operation::Pop);

                  this.emit_force(right);

                  this.point_here(to_end);
                  return;
               },

               node::InfixOperator::Pipe => {
                  this.emit(right);
                  this.emit(left);
               },

               node::InfixOperator::Select => {
                  let scopes = this.scopes.split_off(1);
                  this.emit_scope(right.span(), |this| {
                     // this.scope().push(Span::dummy(), LocalName::wildcard());

                     this.emit(right);
                  });
                  this.scopes.extend(scopes);

                  this.emit_force(left);
                  let to_swap_pop = {
                     this.push_operation(operation.span(), Operation::JumpIfError);
                     this.push_u16(u16::default())
                  };

                  // <right>
                  // <left>
                  this.push_operation(operation.span(), Operation::ScopeSwap);

                  // <right>
                  // <old-scope-or-error>
                  let to_swap_pop_ = {
                     this.push_operation(operation.span(), Operation::JumpIfError);
                     this.push_u16(u16::default())
                  };

                  // <right>
                  // <old-scope>
                  this.push_operation(operation.span(), Operation::Swap);

                  // <old-scope>
                  // <right>
                  this.push_operation(operation.span(), Operation::Force);

                  // <old-scope>
                  // <right-forced>
                  this.point_here(to_swap_pop);
                  this.point_here(to_swap_pop_);
                  this.push_operation(operation.span(), Operation::Swap);

                  // <right-forced>
                  // <old-scope>
                  this.push_operation(operation.span(), Operation::Pop);
                  return;
               },

               node::InfixOperator::And => {
                  this.emit_force(left);
                  let to_right = {
                     this.push_operation(operation.span(), Operation::JumpIf);
                     this.push_u16(u16::default())
                  };
                  let over_right = {
                     this.push_operation(operation.span(), Operation::Jump);
                     this.push_u16(u16::default())
                  };

                  this.point_here(to_right);
                  this.push_operation(operation.span(), Operation::Pop);
                  this.emit_force(right);
                  this.push_operation(operation.span(), Operation::AssertBoolean);

                  this.point_here(over_right);
                  return;
               },

               operator @ (node::InfixOperator::Or | node::InfixOperator::Implication) => {
                  this.emit_force(left);
                  if operator == node::InfixOperator::Implication {
                     this.push_operation(operation.span(), Operation::Not);
                  }

                  let to_end = {
                     this.push_operation(operation.span(), Operation::JumpIf);
                     this.push_u16(u16::default())
                  };

                  this.push_operation(operation.span(), Operation::Pop);
                  this.emit_force(right);
                  this.push_operation(operation.span(), Operation::AssertBoolean);

                  this.point_here(to_end);
                  return;
               },

               node::InfixOperator::Lambda => {
                  this
                     .emit_thunk(operation.span())
                     .is_lambda(true)
                     .with(|this| {
                        this.emit_scope(operation.span(), |this| {
                           // @foo => bar, `@foo` is the right parameter of the equality
                           // comparision, and the left parameter is the
                           // argument.
                           this.emit_force(left);
                           this.push_operation(left.span(), Operation::Equal);

                           let to_body = {
                              this.push_operation(left.span(), Operation::JumpIf);
                              this.push_u16(u16::default())
                           };

                           this.push_operation(operation.span(), Operation::Pop);
                           this.emit_push(
                              left.span(),
                              Value::error(value::string::new!(
                                 "TODO better parameters were not equal error"
                              )),
                           );

                           let over_body = {
                              this.push_operation(operation.span(), Operation::Jump);
                              this.push_u16(u16::default())
                           };

                           this.point_here(to_body);
                           this.push_operation(operation.span(), Operation::Pop);
                           this.emit_force(right);

                           this.point_here(over_body);
                        });
                     });
                  return;
               },

               _ => {
                  this.emit(left);
                  this.emit(right);
               },
            }

            let operation_ = match operation.operator() {
               node::InfixOperator::Sequence => unreachable!("{EXPECT_HANDLED}"),

               node::InfixOperator::ImplicitApply
               | node::InfixOperator::Apply
               | node::InfixOperator::Pipe => Operation::Call,

               node::InfixOperator::Concat => Operation::Concat,
               node::InfixOperator::Construct => Operation::Construct,

               node::InfixOperator::Select => unreachable!("{EXPECT_HANDLED}"),
               node::InfixOperator::Update => Operation::Update,

               node::InfixOperator::LessOrEqual => Operation::LessOrEqual,
               node::InfixOperator::Less => Operation::Less,
               node::InfixOperator::MoreOrEqual => Operation::MoreOrEqual,
               node::InfixOperator::More => Operation::More,

               node::InfixOperator::Equal => Operation::Equal,
               node::InfixOperator::NotEqual => {
                  this.push_operation(operation.span(), Operation::Equal);
                  this.push_operation(operation.span(), Operation::Not);
                  return;
               },

               node::InfixOperator::And
               | node::InfixOperator::Or
               | node::InfixOperator::Implication => unreachable!("{EXPECT_HANDLED}"),

               node::InfixOperator::Same | node::InfixOperator::All => Operation::All,
               node::InfixOperator::Any => Operation::Any,

               node::InfixOperator::Addition => Operation::Addition,
               node::InfixOperator::Subtraction => Operation::Subtraction,
               node::InfixOperator::Multiplication => Operation::Multiplication,
               node::InfixOperator::Power => Operation::Power,
               node::InfixOperator::Division => Operation::Division,

               node::InfixOperator::Lambda => unreachable!("{EXPECT_HANDLED}"),
            };

            this.push_operation(operation.span(), operation_);
         });
   }

   #[expect(unreachable_code)]
   fn emit_suffix_operation(&mut self, operation: &'a node::SuffixOperation) {
      let left = operation.left();

      self
         .emit_thunk(operation.span())
         .is_lambda(left.is_none())
         .with(|this| {
            if let Some(left) = left {
               this.emit(left);
            }

            this.push_operation(operation.span(), match operation.operator() {});
         });
   }

   fn emit_path(&mut self, path: &'a node::Path) {
      let needs_thunk = !path.is_trivial();

      self.emit_thunk(path.span()).if_(needs_thunk).with(|this| {
         let segments = path.segments().into_iter().collect::<SmallVec<_, 4>>();

         for segment in &segments {
            match *segment {
               Segment::Content { span, ref content } => {
                  this.emit_push(
                     span,
                     value::Path::rootless(
                        content
                           .split(value::path::SEPARATOR)
                           .filter(|part| !part.is_empty())
                           .map(value::SString::from)
                           .collect(),
                     ),
                  );
               },

               Segment::Interpolation(interpolation) => {
                  this.emit_scope(interpolation.span(), |this| {
                     this.emit_force(interpolation.expression());
                  });
               },
            }
         }

         if !path.is_trivial() {
            this.push_operation(path.span(), Operation::Interpolate);
            this.push_u64(segments.len() as _);
         }
      });
   }

   #[builder(finish_fn(name = "span"))]
   fn emit_identifier(
      &mut self,
      #[builder(start_fn)] identifier: &'a node::Identifier,
      #[builder(finish_fn)] span: Span,
      #[builder(default)] is_bind: bool,
   ) {
      let needs_thunk =
         // References are always thunked.
         !is_bind ||
         // Binds are thunked if they aren't trivial.
         !identifier.value().is_trivial();

      self.emit_thunk(span).if_(needs_thunk).with(|this| {
         let __name = match identifier.value() {
            node::IdentifierValueRef::Plain(plain) => {
               if is_bind {
                  this.emit_push(span, Value::Bind(value::SString::from(plain.text())));
               } else {
                  this.emit_push(span, Value::Reference(value::SString::from(plain.text())));
                  this.push_operation(span, Operation::Resolve);
               }

               LocalName::plain(plain.text())
            },

            node::IdentifierValueRef::Quoted(quoted) => {
               let segments = quoted.segments().into_iter().collect::<SmallVec<_, 4>>();

               for segment in &segments {
                  match *segment {
                     Segment::Content { span, ref content } => {
                        this.emit_push(
                           span,
                           if is_bind {
                              Value::Bind(value::SString::from(&**content))
                           } else {
                              Value::Reference(value::SString::from(&**content))
                           },
                        );
                     },

                     Segment::Interpolation(interpolation) => {
                        this.emit_scope(interpolation.span(), |this| {
                           this.emit_force(interpolation.expression());
                        });
                     },
                  }
               }

               if !quoted.is_trivial() {
                  this.push_operation(span, Operation::Interpolate);
                  this.push_u64(segments.len() as _);
               }

               if !is_bind {
                  this.push_operation(span, Operation::Resolve);
               }

               LocalName::new(
                  segments
                     .into_iter()
                     .filter_map(|segment| {
                        match segment {
                           Segment::Content { content, .. } => Some(content),

                           Segment::Interpolation(_) => None,
                        }
                     })
                     .collect(),
               )
            },
         };

         // if is_bind {
         //    this.scope().push(span, name);
         //    return;
         // }

         // TODO: Scope logic is wrong. Don't locate it all immediately, do it
         // in scopes. match Scope::locate(&mut this.scopes, &name) {
         //    LocalPosition::Undefined => {
         //       this.reports.push(
         //          Report::warn(if let Ok(name) =
         // TryInto::<&str>::try_into(&name) {
         // format!("undefined reference '{name}'")          } else {
         //             "undefined reference".to_owned()
         //          })
         //          .primary(span, "no definition"),
         //       );
         //    },

         //    mut position => position.mark_used(),
         // }
      });
   }

   fn emit_string(&mut self, string: &'a node::SString) {
      let needs_thunk = !string.is_trivial();

      self
         .emit_thunk(string.span())
         .if_(needs_thunk)
         .with(|this| {
            let segments = string.segments().into_iter().collect::<SmallVec<_, 4>>();

            for segment in &segments {
               match *segment {
                  Segment::Content { span, ref content } => {
                     this.emit_push(span, value::SString::from(&**content));
                  },

                  Segment::Interpolation(interpolation) => {
                     this.emit_scope(interpolation.span(), |this| {
                        this.emit_force(interpolation.expression());
                     });
                  },
               }
            }

            if !string.is_trivial() {
               this.push_operation(string.span(), Operation::Interpolate);
               this.push_u64(segments.len() as _);
            }
         });
   }

   fn emit_if(&mut self, if_: &'a node::If) {
      self.emit_thunk(if_.span()).with(|this| {
         this.emit_force(if_.condition());
         let to_end = {
            this.push_operation(if_.span(), Operation::JumpIfError);
            this.push_u16(u16::default())
         };
         let to_consequence = {
            this.push_operation(if_.span(), Operation::JumpIf);
            this.push_u16(u16::default())
         };
         let to_end_ = {
            this.push_operation(if_.span(), Operation::JumpIfError);
            this.push_u16(u16::default())
         };

         this.push_operation(if_.span(), Operation::Pop);
         this.emit_scope(if_.alternative().span(), |this| {
            this.emit_force(if_.alternative());
         });
         let over_consequence = {
            this.push_operation(if_.span(), Operation::Jump);
            this.push_u16(u16::default())
         };

         this.point_here(to_consequence);
         this.push_operation(if_.span(), Operation::Pop);
         this.emit_scope(if_.consequence().span(), |this| {
            this.emit_force(if_.consequence());
         });

         this.point_here(over_consequence);
         this.point_here(to_end);
         this.point_here(to_end_);
      });
   }

   #[stacksafe::stacksafe]
   fn emit(&mut self, expression: node::ExpressionRef<'a>) {
      match expression {
         node::ExpressionRef::Error(_) => unreachable!("{EXPECT_VALID}"),

         node::ExpressionRef::Parenthesis(parenthesis) => self.emit_parenthesis(parenthesis),

         node::ExpressionRef::List(list) => self.emit_list(list),
         node::ExpressionRef::Attributes(attributes) => self.emit_attributes(attributes),

         node::ExpressionRef::PrefixOperation(prefix_operation) => {
            self.emit_prefix_operation(prefix_operation);
         },
         node::ExpressionRef::InfixOperation(infix_operation) => {
            self.emit_infix_operation(infix_operation);
         },
         node::ExpressionRef::SuffixOperation(suffix_operation) => {
            self.emit_suffix_operation(suffix_operation);
         },

         node::ExpressionRef::Path(path) => self.emit_path(path),

         node::ExpressionRef::Bind(bind) => {
            self
               .emit_identifier(bind.identifier())
               .is_bind(true)
               .span(bind.span());
         },
         node::ExpressionRef::Identifier(identifier) => {
            self.emit_identifier(identifier).span(identifier.span());
         },

         node::ExpressionRef::SString(string) => self.emit_string(string),

         node::ExpressionRef::Char(char) => {
            self.emit_push(char.span(), char.value());
         },
         node::ExpressionRef::Integer(integer) => {
            self.emit_push(integer.span(), Arc::new(integer.value()));
         },
         node::ExpressionRef::Float(float) => {
            self.emit_push(float.span(), float.value());
         },

         node::ExpressionRef::If(if_) => self.emit_if(if_),
      }
   }

   fn emit_force(&mut self, expression: node::ExpressionRef<'a>) {
      self.emit(expression);
      self.push_operation(expression.span(), Operation::Force);
   }
}

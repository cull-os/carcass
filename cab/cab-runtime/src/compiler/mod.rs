use std::sync::Arc;

use cab_report::{
   Report,
   ReportSeverity,
};
use cab_span::{
   IntoSpan as _,
   Span,
};
use cab_syntax::node::{
   self,
   Parted as _,
};

use crate::{
   ByteIndex,
   Code,
   Operation,
   Value,
   ValueIndex,
   value,
};

mod optimizer;

mod scope;
use scope::{
   LocalName,
   LocalPosition,
   Scope,
};

const EXPECT_CODE: &str = "compiler must have at least one code at all times";
const EXPECT_SCOPE: &str = "compiler must have at least one scope at all times";
const EXPECT_VALIDATED: &str = "syntax must be validated";
const EXPECT_HANDLED: &str = "case was handled";

pub struct Compile {
   pub code:    Code,
   pub reports: Vec<Report>,
}

impl Compile {
   pub fn result(self) -> Result<Code, Vec<Report>> {
      if self
         .reports
         .iter()
         .all(|report| report.severity < ReportSeverity::Error)
      {
         Ok(self.code)
      } else {
         Err(self.reports)
      }
   }
}

pub struct Oracle {}

pub fn oracle() -> Oracle {
   Oracle {}
}

impl Oracle {
   pub fn compile(&self, expression: node::ExpressionRef<'_>) -> Compile {
      let mut compiler = Compiler::new();

      compiler.emit_scope(expression.span(), |this| {
         this.emit_force(expression);
      });
      compiler.push_operation(expression.span(), Operation::Return);

      compiler
         .reports
         .sort_by_key(|report| report.labels.iter().map(|label| label.span.start).min());

      Compile {
         code: compiler.codes.pop().expect(EXPECT_CODE),

         reports: compiler.reports,
      }
   }
}

struct Compiler<'a> {
   codes:  Vec<Code>,
   scopes: Vec<Scope<'a>>,

   reports: Vec<Report>,
   dead:    usize,
}

impl<'a> Compiler<'a> {
   fn new() -> Self {
      Compiler {
         codes:   vec![Code::new()],
         scopes:  vec![Scope::global()],
         reports: Vec::new(),

         dead: 0,
      }
   }

   fn code(&mut self) -> &mut Code {
      self.codes.last_mut().expect(EXPECT_CODE)
   }

   fn scope(&mut self) -> &mut Scope<'a> {
      self.scopes.last_mut().expect(EXPECT_SCOPE)
   }

   fn push_u64(&mut self, data: u64) -> ByteIndex {
      if self.dead > 0 {
         return ByteIndex::dummy();
      }

      self.code().push_u64(data)
   }

   fn push_u16(&mut self, data: u16) -> ByteIndex {
      if self.dead > 0 {
         return ByteIndex::dummy();
      }

      self.code().push_u16(data)
   }

   fn push_operation(&mut self, span: Span, operation: Operation) -> ByteIndex {
      if self.dead > 0 {
         return ByteIndex::dummy();
      }

      self.code().push_operation(span, operation)
   }

   fn value(&mut self, value: Value) -> ValueIndex {
      if self.dead > 0 {
         return ValueIndex::dummy();
      }

      self.code().value(value)
   }

   fn point_here(&mut self, index: ByteIndex) {
      if self.dead > 0 {
         return;
      }

      self.code().point_here(index);
   }
}

impl<'a> Compiler<'a> {
   fn emit_push(&mut self, span: Span, value: Value) {
      let index = self.value(value);

      self.push_operation(span, Operation::Push);
      self.push_u64(*index as _);
   }

   fn emit_scope(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      self.scopes.push(Scope::new());

      self.push_operation(span, Operation::ScopeStart);
      closure(self);
      self.push_operation(span, Operation::ScopeEnd);

      for local in self.scopes.pop().expect("scope was just pushed").finish() {
         self.reports.push(
            Report::warn(if let Ok(name) = TryInto::<&str>::try_into(&local.name) {
               format!("unused bind '{name}'")
            } else {
               "unused bind".to_string()
            })
            .primary(local.span, "no usage")
            .tip("remove this or rename it to start with '_'"),
         );
      }
   }

   fn emit_thunk(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      self.codes.push(Code::new());

      closure(self);
      self.push_operation(span, Operation::Return);

      let code = self.codes.pop().expect(EXPECT_CODE);

      self.emit_push(
         span,
         // if code.references_parent {
         Value::Blueprint(Arc::new(code)),
         // } else {
         //     Value::Thunk(Arc::new(Mutex::new(Thunk::suspended(span, context.code))))
         // }
      );
   }

   fn emit_parenthesis(&mut self, parenthesis: &'a node::Parenthesis) {
      self.emit_scope(parenthesis.span(), |this| {
         this.emit(parenthesis.expression().expect(EXPECT_VALIDATED));
      });
   }

   fn emit_list(&mut self, list: &'a node::List) {
      for (index, item) in list.items().enumerate() {
         self.emit_scope(item.span(), |this| this.emit_force(item));

         if index == 0 {
            self.emit_push(list.span(), Value::Nil);
         }

         self.push_operation(list.span(), Operation::Construct);
      }
   }

   fn emit_attributes(&mut self, attributes: &'a node::Attributes) {
      match attributes.expression() {
         Some(expression) => {
            self.emit_thunk(attributes.span(), |this| {
               this.emit_scope(attributes.span(), |this| {
                  this.emit(expression);
                  this.push_operation(expression.span(), Operation::ScopePush);
               });
            });
         },

         None => {
            self.emit_push(attributes.span(), value::Attributes::new().into());
         },
      }
   }

   fn emit_prefix_operation(&mut self, operation: &'a node::PrefixOperation) {
      self.emit_thunk(operation.span(), |this| {
         this.emit(operation.right());

         this.push_operation(operation.span(), match operation.operator() {
            node::PrefixOperator::Swwallation => Operation::Swwallation,
            node::PrefixOperator::Negation => Operation::Negation,
            node::PrefixOperator::Not => Operation::Not,
         });
      });
   }

   fn emit_infix_operation(&mut self, operation: &'a node::InfixOperation) {
      self.emit_thunk(operation.span(), |this| {
         match operation.operator() {
            node::InfixOperator::Sequence => {
               this.emit_force(operation.left());
               this.push_operation(operation.span(), Operation::Pop);

               this.emit(operation.right());
               return;
            },

            node::InfixOperator::Pipe => {
               this.emit(operation.right());
               this.emit(operation.left());
            },

            node::InfixOperator::Select => {
               let scopes = this.scopes.split_off(1);

               this.emit_scope(operation.right().span(), |this| {
                  this.scope().push(Span::dummy(), LocalName::wildcard());

                  this.emit(operation.right())
               });

               this.scopes.extend(scopes);

               this.emit(operation.left());

               // <right>
               // <left>
               this.push_operation(operation.span(), Operation::ScopeSwap);

               // <right>
               // <old-scope>
               this.push_operation(operation.span(), Operation::Swap);
               this.push_u16(1);

               // <old-scope>
               // <right>
               this.push_operation(operation.span(), Operation::Force);

               // <old-scope>
               // <right-forced>
               this.push_operation(operation.span(), Operation::Swap);
               this.push_u16(1);

               // <right-forced>
               // <old-scope>
               this.push_operation(operation.span(), Operation::Pop);

               // <right-forced>
               return;
            },

            node::InfixOperator::And => {
               this.emit_force(operation.left());
               let to_right = {
                  this.push_operation(operation.span(), Operation::JumpIf);
                  this.push_u16(0)
               };
               let over_right = {
                  this.push_operation(operation.span(), Operation::Jump);
                  this.push_u16(0)
               };

               this.point_here(to_right);
               this.push_operation(operation.span(), Operation::Pop);
               this.emit_force(operation.right());
               this.push_operation(operation.span(), Operation::AssertBoolean);

               this.point_here(over_right);
               return;
            },

            operator @ (node::InfixOperator::Or | node::InfixOperator::Implication) => {
               this.emit_force(operation.left());
               if operator == node::InfixOperator::Implication {
                  this.push_operation(operation.span(), Operation::Not);
               }

               let to_end = {
                  this.push_operation(operation.span(), Operation::JumpIf);
                  this.push_u16(0)
               };

               this.push_operation(operation.span(), Operation::Pop);
               this.emit_force(operation.right());
               this.push_operation(operation.span(), Operation::AssertBoolean);

               this.point_here(to_end);
               return;
            },

            _ => {
               this.emit(operation.left());
               this.emit(operation.right());
            },
         }

         let operation_ = match operation.operator() {
            node::InfixOperator::Sequence => unreachable!("{EXPECT_HANDLED}"),

            node::InfixOperator::ImplicitApply
            | node::InfixOperator::Apply
            | node::InfixOperator::Pipe => todo!(),

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

            node::InfixOperator::Lambda => todo!(),
         };

         this.push_operation(operation.span(), operation_);
      });
   }

   fn emit_suffix_operation(&mut self, operation: &'a node::SuffixOperation) {
      match operation.operator() {
         node::SuffixOperator::Same => self.emit(operation.left()),
      }
   }

   // fn emit_island(&mut self, island: &node::PathRoot) {
   //    self.emit_thunk(island.span(), |this| {
   //       let parts = island
   //          .type_()
   //          .parts()
   //          .filter(|part| !part.is_delimiter())
   //          .collect::<Vec<_>>();

   //       if parts.len() != 1 || !parts[0].is_content() {
   //          this
   //             .code
   //             .push_operation(island.span(),
   // Operation::IslandHeaderInterpolate);          this.code.push_u64(parts.
   // len() as _);       }

   //       for part in parts {
   //          match part {
   //             node::InterpolatedPartRef::Content(content) => {
   //                this.emit_push(content.span(),
   // Value::IslandHeader(content.text().into()));             },

   //             node::InterpolatedPartRef::Interpolation(interpolation) => {
   //                this.emit_scope(interpolation.span(), |this| {
   //                   this.emit_force(interpolation.expression());
   //                })
   //             },

   //             _ => {},
   //          }
   //       }

   //       if let Some(config) = island.config() {
   //          this.emit_scope(config.span(), |this| this.emit_force(config));
   //       } else {
   //          this.emit_push(
   //             island.span(),
   //
   // Value::Attributes(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher)),
   //          );
   //       }

   //       if let Some(path) = island.path() {
   //          this.emit_scope(path.span(), |this| this.emit_force(path));
   //       } else {
   //          this.emit_push(island.span(), Value::Path("/".into()));
   //       }

   //       this.code.push_operation(island.span(), Operation::Island);
   //    });
   // }

   // fn emit_path(&mut self, path: &node::Path) {
   //    self.emit_thunk(path.span(), |this| {
   //       let parts = path
   //          .parts()
   //          .filter(|part| !part.is_delimiter())
   //          .collect::<Vec<_>>();

   //       if parts.len() != 1 || !parts[0].is_content() {
   //          this
   //             .code
   //             .push_operation(path.span(), Operation::PathInterpolate);
   //          this.code.push_u64(parts.len() as _);
   //       }

   //       for part in parts {
   //          match part {
   //             node::InterpolatedPartRef::Content(content) => {
   //                this.emit_push(content.span(),
   // Value::Path(content.text().into()));             },

   //             node::InterpolatedPartRef::Interpolation(interpolation) => {
   //                this.emit_scope(interpolation.span(), |this| {
   //                   this.emit_force(interpolation.expression());
   //                });
   //             },

   //             _ => {},
   //          }
   //       }
   //    });
   // }

   fn emit_identifier(&mut self, is_bind: bool, span: Span, identifier: &'a node::Identifier) {
      self.emit_thunk(span, |this| {
         let name = match identifier.value() {
            node::IdentifierValueRef::Plain(plain) => {
               if is_bind {
                  this.emit_push(span, Value::Bind(plain.text().into()));
               } else {
                  this.emit_push(span, Value::Reference(plain.text().into()));
                  this.push_operation(span, Operation::Resolve);
               }

               LocalName::plain(plain.text())
            },

            node::IdentifierValueRef::Quoted(quoted) => {
               let parts = quoted
                  .parts()
                  .filter(|part| !part.is_delimiter())
                  .collect::<Vec<_>>();

               for part in &parts {
                  match part {
                     node::InterpolatedPartRef::Content(content) => {
                        this.emit_push(
                           content.span(),
                           if is_bind {
                              Value::Bind(content.text().into())
                           } else {
                              Value::Reference(content.text().into())
                           },
                        );
                     },

                     node::InterpolatedPartRef::Interpolation(interpolation) => {
                        this.emit_scope(interpolation.span(), |this| {
                           this.emit_force(interpolation.expression());
                        });
                     },

                     _ => {},
                  }
               }

               if parts.len() != 1 || !parts[0].is_content() {
                  this.push_operation(span, Operation::Interpolate);
                  this.push_u64(parts.len() as _);
               }

               if !is_bind {
                  this.push_operation(span, Operation::Resolve);
               }

               LocalName::new(
                  parts
                     .into_iter()
                     .filter_map(|part| {
                        match part {
                           node::InterpolatedPartRef::Content(content) => Some(content.text()),

                           _ => None,
                        }
                     })
                     .collect(),
               )
            },
         };

         if is_bind {
            this.scope().push(span, name);
            return;
         }

         match Scope::locate(&mut this.scopes, &name) {
            LocalPosition::Undefined => {
               this.reports.push(
                  Report::warn(if let Ok(name) = TryInto::<&str>::try_into(&name) {
                     format!("undefined reference '{name}'")
                  } else {
                     "undefined reference".to_owned()
                  })
                  .primary(span, "no definition"),
               )
            },

            mut position => position.mark_used(),
         }
      });
   }

   fn emit_if(&mut self, if_: &'a node::If) {
      self.emit_thunk(if_.span(), |this| {
         this.emit_force(if_.condition());
         let to_consequence = {
            this.push_operation(if_.span(), Operation::JumpIf);
            this.push_u16(0)
         };

         this.push_operation(if_.span(), Operation::Pop);
         this.emit(if_.alternative());
         let over_consequence = {
            this.push_operation(if_.span(), Operation::Jump);
            this.push_u16(0)
         };

         this.point_here(to_consequence);
         this.push_operation(if_.span(), Operation::Pop);
         this.emit(if_.alternative());

         this.point_here(over_consequence);
      });
   }

   fn emit(&mut self, expression: node::ExpressionRef<'a>) {
      let expression = self.optimize(expression);

      match expression {
         node::ExpressionRef::Error(_) => unreachable!("{EXPECT_VALIDATED}"),

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

         node::ExpressionRef::Path(_path) => todo!(),

         node::ExpressionRef::Bind(bind) => {
            let node::ExpressionRef::Identifier(identifier) = bind.identifier() else {
               unreachable!("{EXPECT_VALIDATED}")
            };
            self.emit_identifier(true, bind.span(), identifier);
         },
         node::ExpressionRef::Identifier(identifier) => {
            self.emit_identifier(false, identifier.span(), identifier);
         },

         node::ExpressionRef::SString(_string) => todo!(),

         node::ExpressionRef::Rune(rune) => {
            self.emit_push(rune.span(), Value::Rune(rune.value()));
         },
         node::ExpressionRef::Integer(integer) => {
            self.emit_push(integer.span(), Value::Integer(integer.value()));
         },
         node::ExpressionRef::Float(float) => {
            self.emit_push(float.span(), Value::Float(float.value()));
         },

         node::ExpressionRef::If(if_) => self.emit_if(if_),
      }
   }

   fn emit_dead(&mut self, expression: node::ExpressionRef<'a>) {
      self.dead += 1;
      self.emit(expression);
      self.dead -= 1;
   }

   fn emit_force(&mut self, expression: node::ExpressionRef<'a>) {
      self.emit(expression);
      self.push_operation(expression.span(), Operation::Force);
   }
}

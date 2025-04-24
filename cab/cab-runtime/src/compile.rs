use std::{
   cell::RefCell,
   mem,
   ops,
   rc::Rc,
   sync::{
      Arc,
      Mutex,
   },
};

use cab_syntax::node::{
   self,
   Parted as _,
};
use cab_why::{
   IntoSpan as _,
   Report,
   ReportSeverity,
   Span,
};
use rpds::HashTrieMapSync as HashTrieMap;
use rustc_hash::FxBuildHasher;

use crate::{
   Code,
   LocalName,
   LocalPosition,
   Operation,
   Scope,
   Thunk,
   Value,
};

const EXPECT_CONTEXT: &str = "compiler must have at least one context at all times";
const EXPECT_VALIDATED: &str = "syntax must be validated";

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

      Compile {
         code: compiler.contexts.pop().expect(EXPECT_CONTEXT).code,

         reports: compiler.reports,
      }
   }
}

struct Context {
   code:  Code,
   scope: Rc<RefCell<Scope>>,
}

struct Compiler {
   contexts: Vec<Context>,
   reports:  Vec<Report>,
}

impl ops::Deref for Compiler {
   type Target = Context;

   fn deref(&self) -> &Self::Target {
      self.contexts.last().expect(EXPECT_CONTEXT)
   }
}

impl ops::DerefMut for Compiler {
   fn deref_mut(&mut self) -> &mut Self::Target {
      self.contexts.last_mut().expect(EXPECT_CONTEXT)
   }
}

impl Compiler {
   fn new() -> Self {
      Compiler {
         contexts: vec![Context {
            code:  Code::new(),
            scope: Rc::new(RefCell::new(Scope::root())),
         }],

         reports: Vec::new(),
      }
   }

   fn emit_push(&mut self, span: Span, value: Value) {
      let index = self.code.push_value(value);

      self.code.push_operation(span, Operation::Push);
      self.code.push_u64(*index as _);
   }

   fn emit_scope(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      let mut scope_new = Rc::new(RefCell::new(Scope::new(&self.scope)));

      mem::swap(&mut scope_new, &mut self.scope);
      let mut scope_old = scope_new;

      self.code.push_operation(span, Operation::ScopeStart);
      closure(self);
      self.code.push_operation(span, Operation::ScopeEnd);

      mem::swap(&mut scope_old, &mut self.scope);
      let scope_new = scope_old;

      for local in scope_new.borrow().finish() {
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
      self.contexts.push(Context {
         code:  Code::new(),
         scope: self.scope.clone(),
      });

      closure(self);
      self.code.push_operation(span, Operation::Return);

      let context = self.contexts.pop().expect(EXPECT_CONTEXT);

      self.emit_push(
         span,
         // if context.scope.borrow().references_parent {
         //    Value::Blueprint(Arc::new(context.code))
         // } else {
         Value::Thunk(Arc::new(Mutex::new(Thunk::suspended(span, context.code)))), // },
      );
   }

   fn emit_parenthesis(&mut self, parenthesis: &node::Parenthesis) {
      self.emit_scope(parenthesis.span(), |this| {
         this.emit(parenthesis.expression().expect(EXPECT_VALIDATED));
      });
   }

   fn emit_list(&mut self, list: &node::List) {
      self.emit_push(list.span(), Value::Nil);

      for item in list.items() {
         self.emit_scope(item.span(), |this| this.emit_force(item));

         self.code.push_operation(list.span(), Operation::Construct);
      }
   }

   fn emit_attributes(&mut self, attributes: &node::Attributes) {
      if let Some(expression) = attributes.expression() {
         self.emit_thunk(attributes.span(), |this| {
            this.emit_scope(attributes.span(), |this| this.emit(expression));
            this
               .code
               .push_operation(expression.span(), Operation::PushScope);
         });
      } else {
         self.emit_push(
            attributes.span(),
            Value::Attributes(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher)),
         );
      }
   }

   fn emit_prefix_operation(&mut self, operation: &node::PrefixOperation) {
      self.emit_thunk(operation.span(), |this| {
         this
            .code
            .push_operation(operation.span(), match operation.operator() {
               node::PrefixOperator::Swwallation => Operation::Swwallation,
               node::PrefixOperator::Negation => Operation::Negation,
               node::PrefixOperator::Not => Operation::Not,
               node::PrefixOperator::Try => Operation::Try,
            });

         this.emit_force(operation.right());
      });
   }

   fn emit_infix_operation(&mut self, operation: &node::InfixOperation) {
      self.emit_thunk(operation.span(), |this| {
         let operation_ = match operation.operator() {
            node::InfixOperator::Same => Operation::Same,
            node::InfixOperator::Sequence => Operation::Sequence,

            node::InfixOperator::ImplicitApply
            | node::InfixOperator::Apply
            | node::InfixOperator::Pipe => Operation::Apply,

            node::InfixOperator::Concat => Operation::Concat,
            node::InfixOperator::Construct => Operation::Construct,

            node::InfixOperator::Select => Operation::Select,
            node::InfixOperator::Update => Operation::Update,

            node::InfixOperator::LessOrEqual => Operation::LessOrEqual,
            node::InfixOperator::Less => Operation::Less,
            node::InfixOperator::MoreOrEqual => Operation::MoreOrEqual,
            node::InfixOperator::More => Operation::More,

            node::InfixOperator::Equal => Operation::Equal,
            node::InfixOperator::NotEqual => {
               this.code.push_operation(operation.span(), Operation::Not);
               Operation::Equal
            },

            node::InfixOperator::And => Operation::And,
            node::InfixOperator::Or => Operation::Or,
            node::InfixOperator::Implication => Operation::Implication,

            node::InfixOperator::All => Operation::All,
            node::InfixOperator::Any => Operation::Any,

            node::InfixOperator::Addition => Operation::Addition,
            node::InfixOperator::Subtraction => Operation::Subtraction,
            node::InfixOperator::Multiplication => Operation::Multiplication,
            node::InfixOperator::Power => Operation::Power,
            node::InfixOperator::Division => Operation::Division,

            node::InfixOperator::Lambda => todo!(),
         };

         this.code.push_operation(operation.span(), operation_);

         if operation.operator() == node::InfixOperator::Pipe {
            this.emit_force(operation.right());
            this.emit_force(operation.left());
         } else {
            this.emit_force(operation.left());
            this.emit_force(operation.right());
         }
      });
   }

   fn emit_suffix_operation(&mut self, operation: &node::SuffixOperation) {
      match operation.operator() {
         node::SuffixOperator::Same => self.emit(operation.left()),
         node::SuffixOperator::Sequence => {
            self.emit_thunk(operation.span(), |this| {
               this
                  .code
                  .push_operation(operation.span(), Operation::Sequence);

               this.emit_force(operation.left());

               // TODO: Use a proper value.
               this.emit_push(operation.span(), Value::Nil);
            });
         },
      }
   }

   fn emit_island(&mut self, island: &node::Island) {
      self.emit_thunk(island.span(), |this| {
         let parts = island
            .header()
            .parts()
            .filter(|part| !part.is_delimiter())
            .collect::<Vec<_>>();

         if parts.len() != 1 || !parts[0].is_content() {
            this
               .code
               .push_operation(island.span(), Operation::IslandHeaderInterpolate);
            this.code.push_u64(parts.len() as _);
         }

         for part in parts {
            match part {
               node::InterpolatedPartRef::Content(content) => {
                  this.emit_push(content.span(), Value::IslandHeader(content.text().into()));
               },

               node::InterpolatedPartRef::Interpolation(interpolation) => {
                  this.emit_scope(interpolation.span(), |this| {
                     this.emit_force(interpolation.expression());
                  })
               },

               _ => {},
            }
         }

         if let Some(config) = island.config() {
            this.emit_scope(config.span(), |this| this.emit_force(config));
         } else {
            this.emit_push(
               island.span(),
               Value::Attributes(HashTrieMap::new_with_hasher_and_ptr_kind(FxBuildHasher)),
            );
         }

         if let Some(path) = island.path() {
            this.emit_scope(path.span(), |this| this.emit_force(path));
         } else {
            this.emit_push(island.span(), Value::Path("/".into()));
         }

         this.code.push_operation(island.span(), Operation::Island);
      });
   }

   fn emit_path(&mut self, path: &node::Path) {
      self.emit_thunk(path.span(), |this| {
         let parts = path
            .parts()
            .filter(|part| !part.is_delimiter())
            .collect::<Vec<_>>();

         if parts.len() != 1 || !parts[0].is_content() {
            this
               .code
               .push_operation(path.span(), Operation::PathInterpolate);
            this.code.push_u64(parts.len() as _);
         }

         for part in parts {
            match part {
               node::InterpolatedPartRef::Content(content) => {
                  this.emit_push(content.span(), Value::Path(content.text().into()));
               },

               node::InterpolatedPartRef::Interpolation(interpolation) => {
                  this.emit_scope(interpolation.span(), |this| {
                     this.emit_force(interpolation.expression());
                  });
               },

               _ => {},
            }
         }
      });
   }

   fn emit_identifier(&mut self, is_bind: bool, span: Span, identifier: &node::Identifier) {
      self.emit_thunk(span, |this| {
         let name = match identifier.value() {
            node::IdentifierValueRef::Plain(plain) => {
               if is_bind {
                  this.emit_push(span, Value::Bind(plain.text().into()));
               } else {
                  this.code.push_operation(span, Operation::GetLocal);
                  this.emit_push(span, Value::Identifier(plain.text().into()));
               }

               LocalName::new(vec![plain.text().to_owned()])
            },

            node::IdentifierValueRef::Quoted(quoted) => {
               let parts = quoted
                  .parts()
                  .filter(|part| !part.is_delimiter())
                  .collect::<Vec<_>>();

               if parts.len() != 1 || !parts[0].is_content() {
                  this.code.push_operation(
                     span,
                     if is_bind {
                        Operation::BindInterpolate
                     } else {
                        Operation::IdentifierInterpolate
                     },
                  );
                  this.code.push_u64(parts.len() as _);
               }

               for part in &parts {
                  match part {
                     node::InterpolatedPartRef::Content(content) => {
                        this.emit_push(
                           content.span(),
                           if is_bind {
                              Value::Bind(content.text().into())
                           } else {
                              Value::Identifier(content.text().into())
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

               LocalName::new(
                  parts
                     .into_iter()
                     .filter_map(|part| {
                        match part {
                           node::InterpolatedPartRef::Content(content) => {
                              Some(content.text().to_owned())
                           },

                           _ => None,
                        }
                     })
                     .collect(),
               )
            },
         };

         if is_bind {
            this.scope.borrow_mut().push(span, name);
            return;
         }

         match Scope::locate(&this.scope, &name) {
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

            position => position.mark_used(),
         }
      });
   }

   fn emit(&mut self, expression: node::ExpressionRef<'_>) {
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

         node::ExpressionRef::Island(island) => self.emit_island(island),
         node::ExpressionRef::Path(path) => self.emit_path(path),

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

         node::ExpressionRef::If(_) => todo!(),
      }
   }

   fn emit_force(&mut self, expression: node::ExpressionRef<'_>) {
      self
         .code
         .push_operation(expression.span(), Operation::Force);
      self.emit(expression);
   }
}

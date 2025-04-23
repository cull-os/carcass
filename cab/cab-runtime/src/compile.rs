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
use indexmap::IndexMap;
use rustc_hash::FxBuildHasher;

use crate::{
   Code,
   LocalName,
   Operation,
   Scope,
   Thunk,
   Value,
};

const CONTEXT_EXPECT: &str = "compiler must have at least one context at all times";

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

      compiler.scope(expression.span(), |this| {
         this.emit(expression);
      });

      Compile {
         code: compiler.contexts.pop().expect(CONTEXT_EXPECT).code,

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
      self.contexts.last().expect(CONTEXT_EXPECT)
   }
}

impl ops::DerefMut for Compiler {
   fn deref_mut(&mut self) -> &mut Self::Target {
      self.contexts.last_mut().expect(CONTEXT_EXPECT)
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

   fn context(&mut self, closure: impl FnOnce(&mut Self)) -> Context {
      self.contexts.push(Context {
         code:  Code::new(),
         scope: Rc::clone(&self.scope),
      });

      closure(self);

      self.contexts.pop().expect(CONTEXT_EXPECT)
   }

   fn scope(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      let mut scope_new = Rc::new(RefCell::new(Scope::new(&self.scope)));

      mem::swap(&mut scope_new, &mut self.scope);
      let mut scope_old = scope_new;

      self.code.push_operation(span, Operation::Scope);
      closure(self);

      mem::swap(&mut scope_old, &mut self.scope);
      let scope_new = scope_old;

      for local in scope_new.borrow().all_unused() {
         self.reports.push(
            Report::warn(if let LocalName::Static(name) = &local.name {
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
      let mut context = self.context(|this| closure(this));

      context.code.push_operation(span, Operation::Return);

      self.code.push_value(
         span,
         if context.scope.borrow().is_self_contained() {
            Value::Thunk(Arc::new(Mutex::new(Thunk::suspended(span, context.code))))
         } else {
            Value::Blueprint(Arc::new(context.code))
         },
      );
   }

   fn emit_parenthesis(&mut self, parenthesis: &node::Parenthesis) {
      self.scope(parenthesis.span(), |this| {
         this.emit(parenthesis.expression().expect("node must be validated"));
      });
   }

   fn emit_list(&mut self, list: &node::List) {
      for item in list.items() {
         self.code.push_operation(list.span(), Operation::Construct);

         self.scope(item.span(), |this| this.emit(item));
      }

      self.code.push_value(list.span(), Value::Nil);
   }

   fn emit_attributes(&mut self, attributes: &node::Attributes) {
      if let Some(expression) = attributes.expression() {
         self.emit_thunk(attributes.span(), |this| {
            this
               .code
               .push_operation(expression.span(), Operation::Attributes);
            this.emit(expression);
         });
      } else {
         self.code.push_value(
            attributes.span(),
            Value::Attributes(Arc::new(IndexMap::with_hasher(FxBuildHasher))),
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

         this
            .code
            .push_operation(operation.right().span(), Operation::Force);
         this.emit(operation.right());
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
            this
               .code
               .push_operation(operation.right().span(), Operation::Force);
            this.emit(operation.right());
            this
               .code
               .push_operation(operation.left().span(), Operation::Force);
            this.emit(operation.left());
         } else {
            this
               .code
               .push_operation(operation.left().span(), Operation::Force);
            this.emit(operation.left());
            this
               .code
               .push_operation(operation.right().span(), Operation::Force);
            this.emit(operation.right());
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

               this
                  .code
                  .push_operation(operation.left().span(), Operation::Force);
               this.emit(operation.left());

               // TODO: Use a proper value.
               this.code.push_value(operation.span(), Value::Nil);
            });
         },
      }
   }

   fn emit_path(&mut self, path: &node::Path) {
      self.emit_thunk(path.span(), |this| {
         let parts = path
            .parts()
            .filter(|part| !part.is_delimiter())
            .collect::<Vec<_>>();

         if parts.len() > 1 || !parts[0].is_content() {
            this
               .code
               .push_operation(path.span(), Operation::PathInterpolate);
            this.code.push_u64(parts.len() as _);
         }

         for part in parts {
            match part {
               node::InterpolatedPartRef::Content(content) => {
                  this
                     .code
                     .push_value(content.span(), Value::Path(content.text().to_owned()));
               },

               node::InterpolatedPartRef::Interpolation(interpolation) => {
                  this.scope(interpolation.span(), |this| {
                     this
                        .code
                        .push_operation(interpolation.span(), Operation::Force);
                     this.emit(interpolation.expression());
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
                  this
                     .code
                     .push_value(span, Value::Bind(plain.text().to_owned()));
               } else {
                  this.code.push_operation(span, Operation::GetLocal);
                  this
                     .code
                     .push_value(span, Value::Identifier(plain.text().to_owned()));
               }

               LocalName::Static(plain.text().to_owned())
            },

            node::IdentifierValueRef::Quoted(quoted) => {
               let parts = quoted
                  .parts()
                  .filter(|part| !part.is_delimiter())
                  .collect::<Vec<_>>();

               if parts.len() > 1 || !parts[0].is_content() {
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
                        this.code.push_value(
                           content.span(),
                           if is_bind {
                              Value::Bind(content.text().to_owned())
                           } else {
                              Value::Identifier(content.text().to_owned())
                           },
                        );
                     },

                     node::InterpolatedPartRef::Interpolation(interpolation) => {
                        this.scope(interpolation.span(), |this| {
                           this
                              .code
                              .push_operation(interpolation.span(), Operation::Force);
                           this.emit(interpolation.expression());
                        });
                     },

                     _ => {},
                  }
               }

               if parts.len() == 1
                  && let node::InterpolatedPartRef::Content(content) = parts[0]
               {
                  LocalName::Static(content.text().to_owned())
               } else {
                  LocalName::Dynamic(
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
               }
            },
         };

         if is_bind {
            this.scope.borrow_mut().push(span, name);
            return;
         }

         let LocalName::Static(literal) = name else {
            this.scope.borrow_mut().mark_all_used();
            return;
         };

         let Some((scope, index)) = Scope::resolve(&this.scope, &literal) else {
            this.reports.push(
               Report::warn(format!("undefined reference '{literal}'"))
                  .primary(span, "no definition"),
            );
            return;
         };

         match index {
            Some(index) => {
               scope.borrow_mut().mark_used(index);
            },

            None => {
               scope.borrow_mut().mark_all_used();
            },
         }
      });
   }

   fn emit(&mut self, expression: node::ExpressionRef<'_>) {
      match expression {
         node::ExpressionRef::Error(_) => unreachable!(),

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

         node::ExpressionRef::Island(_island) => todo!(),
         node::ExpressionRef::Path(path) => self.emit_path(path),

         node::ExpressionRef::Bind(bind) => {
            let node::ExpressionRef::Identifier(identifier) = bind.identifier() else {
               unreachable!()
            };
            self.emit_identifier(true, bind.span(), identifier);
         },
         node::ExpressionRef::Identifier(identifier) => {
            self.emit_identifier(false, identifier.span(), identifier);
         },

         node::ExpressionRef::SString(_string) => todo!(),

         node::ExpressionRef::Rune(rune) => {
            self.code.push_value(rune.span(), Value::Rune(rune.value()));
         },
         node::ExpressionRef::Integer(integer) => {
            self
               .code
               .push_value(integer.span(), Value::Integer(integer.value()));
         },
         node::ExpressionRef::Float(float) => {
            self
               .code
               .push_value(float.span(), Value::Float(float.value()));
         },

         node::ExpressionRef::If(_) => todo!(),
      }
   }
}

use std::{
   cell::RefCell,
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
   pub fn compile(&self, node: node::ExpressionRef<'_>) -> Compile {
      let mut compiler = Compiler::new();

      compiler.emit(node);

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
   type Target = Code;

   fn deref(&self) -> &Self::Target {
      &self.contexts.last().expect(CONTEXT_EXPECT).code
   }
}

impl ops::DerefMut for Compiler {
   fn deref_mut(&mut self) -> &mut Self::Target {
      &mut self.contexts.last_mut().expect(CONTEXT_EXPECT).code
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

   fn scope(&self) -> &Rc<RefCell<Scope>> {
      &self.contexts.last().expect(CONTEXT_EXPECT).scope
   }

   fn context(&mut self, closure: impl FnOnce(&mut Self)) -> Context {
      self.contexts.push(Context {
         code:  Code::new(),
         scope: Rc::new(RefCell::new(Scope::new(self.scope()))),
      });

      closure(self);

      self.contexts.pop().expect(CONTEXT_EXPECT)
   }

   fn emit_thunk(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      let mut context = self.context(|this| closure(this));

      context.code.push_operation(span, Operation::Return);

      if context.scope.borrow().is_self_contained() {
         self.push_value(
            span,
            Value::Thunk(Arc::new(Mutex::new(Thunk::suspended(span, context.code)))),
         );
         return;
      }

      self.push_value(span, Value::Blueprint(Arc::new(context.code)));
   }

   fn emit_parenthesis(&mut self, parenthesis: &node::Parenthesis) {
      self.push_operation(parenthesis.span(), Operation::Scope);
      self.emit(parenthesis.expression().expect("node must be validated"));
   }

   fn emit_list(&mut self, list: &node::List) {
      for item in list.items() {
         self.push_operation(list.span(), Operation::Construct);

         self.push_operation(list.span(), Operation::Scope);
         self.emit(item);
      }

      self.push_value(list.span(), Value::Nil);
   }

   fn emit_attributes(&mut self, attributes: &node::Attributes) {
      if let Some(expression) = attributes.expression() {
         self.emit_thunk(attributes.span(), |this| {
            this.push_operation(expression.span(), Operation::Attributes);
            this.emit(expression);
         });
      } else {
         self.push_value(
            attributes.span(),
            Value::Attributes(Arc::new(IndexMap::with_hasher(FxBuildHasher))),
         );
      }
   }

   fn emit_prefix_operation(&mut self, operation: &node::PrefixOperation) {
      self.emit_thunk(operation.span(), |this| {
         this.push_operation(operation.span(), match operation.operator() {
            node::PrefixOperator::Swwallation => Operation::Swwallation,
            node::PrefixOperator::Negation => Operation::Negation,
            node::PrefixOperator::Not => Operation::Not,
            node::PrefixOperator::Try => Operation::Try,
         });

         this.push_operation(operation.right().span(), Operation::Force);
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
               this.push_operation(operation.span(), Operation::Not);
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

         this.push_operation(operation.span(), operation_);

         if operation.operator() == node::InfixOperator::Pipe {
            this.push_operation(operation.right().span(), Operation::Force);
            this.emit(operation.right());
            this.push_operation(operation.left().span(), Operation::Force);
            this.emit(operation.left());
         } else {
            this.push_operation(operation.left().span(), Operation::Force);
            this.emit(operation.left());
            this.push_operation(operation.right().span(), Operation::Force);
            this.emit(operation.right());
         }
      });
   }

   fn emit_suffix_operation(&mut self, operation: &node::SuffixOperation) {
      match operation.operator() {
         node::SuffixOperator::Same => self.emit(operation.left()),
         node::SuffixOperator::Sequence => {
            self.emit_thunk(operation.span(), |this| {
               this.push_operation(operation.span(), Operation::Sequence);

               this.push_operation(operation.left().span(), Operation::Force);
               this.emit(operation.left());

               // TODO: Use a proper value.
               this.push_value(operation.span(), Value::Nil);
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
            this.push_operation(path.span(), Operation::PathInterpolate);
            this.push_u64(parts.len() as _);
         }

         for part in parts {
            match part {
               node::InterpolatedPartRef::Content(content) => {
                  this.push_value(content.span(), Value::Path(content.text().to_owned()));
               },

               node::InterpolatedPartRef::Interpolation(interpolation) => {
                  this.push_operation(interpolation.span(), Operation::Scope);
                  this.push_operation(interpolation.span(), Operation::Force);
                  this.emit(interpolation.expression());
               },

               _ => {},
            }
         }
      });
   }

   fn emit_reference(&mut self, identifier: &node::Identifier) {
      let literal = match identifier.value() {
         node::IdentifierValueRef::Plain(plain) => {
            self.push_operation(plain.span(), Operation::GetLocal);
            self.push_value(
               identifier.span(),
               Value::Identifier(plain.text().to_owned()),
            );

            Some(plain.text())
         },

         node::IdentifierValueRef::Quoted(quoted) => {
            let parts = quoted
               .parts()
               .filter(|part| !part.is_delimiter())
               .collect::<Vec<_>>();

            if parts.len() > 1 || !parts[0].is_content() {
               self.push_operation(quoted.span(), Operation::IdentifierInterpolate);
               self.push_u64(parts.len() as _);
            }

            for part in &parts {
               match part {
                  node::InterpolatedPartRef::Content(content) => {
                     self.push_value(content.span(), Value::Identifier(content.text().to_owned()));
                  },

                  node::InterpolatedPartRef::Interpolation(interpolation) => {
                     self.push_operation(interpolation.span(), Operation::Scope);
                     self.push_operation(interpolation.span(), Operation::Force);
                     self.emit(interpolation.expression());
                  },

                  _ => {},
               }
            }

            if parts.len() == 1
               && let node::InterpolatedPartRef::Content(content) = parts[0]
            {
               Some(content.text())
            } else {
               None
            }
         },
      };

      match literal {
         Some(name) => {
            let Some((scope, index)) = Scope::resolve(self.scope(), name) else {
               self.reports.push(
                  Report::warn("undefined variable").primary(identifier.span(), "no definition"),
               );
               return;
            };

            match index {
               Some(index) => {
                  scope.borrow_mut().locals[*index].used = true;
               },

               None => {
                  let scope = &mut *scope.borrow_mut();

                  for index in scope.by_name.values() {
                     scope.locals[**index].used = true;
                  }
               },
            }
         },

         None => {
            let scope = &mut *self.scope().borrow_mut();

            for index in scope.by_name.values() {
               scope.locals[**index].used = true;
            }
         },
      }
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
         node::ExpressionRef::Bind(_bind) => todo!(),
         node::ExpressionRef::Identifier(identifier) => self.emit_reference(identifier),
         node::ExpressionRef::SString(_string) => todo!(),

         node::ExpressionRef::Rune(rune) => {
            self.push_value(rune.span(), Value::Rune(rune.value()));
         },
         node::ExpressionRef::Integer(integer) => {
            self.push_value(integer.span(), Value::Integer(integer.value()));
         },
         node::ExpressionRef::Float(float) => {
            self.push_value(float.span(), Value::Float(float.value()));
         },

         node::ExpressionRef::If(_) => todo!(),
      }
   }
}

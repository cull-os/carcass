use std::{
   ops,
   sync::Arc,
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
use rustc_hash::FxHasher;

use crate::{
   Code,
   Constant,
   Operation,
};

const CODE_EXPECT: &str = "compiler must have at least one code at all times";

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
         code: compiler.contexts.pop().expect(CODE_EXPECT).code,

         reports: compiler.reports,
      }
   }
}

struct Context {
   code:  Code,
   scope: Scope,
}

struct Compiler {
   contexts: Vec<Context>,
   reports:  Vec<Report>,
}

impl ops::Deref for Compiler {
   type Target = Code;

   fn deref(&self) -> &Self::Target {
      &self.contexts.last().expect(CODE_EXPECT).code
   }
}

impl ops::DerefMut for Compiler {
   fn deref_mut(&mut self) -> &mut Self::Target {
      &mut self.contexts.last_mut().expect(CODE_EXPECT).code
   }
}

impl Compiler {
   fn new() -> Self {
      Compiler {
         contexts: Vec::new(),
         reports:  Vec::new(),
      }
   }

   fn context(&mut self, closure: impl FnOnce(&mut Self)) -> Context {
      self.contexts.push(Context {
         code:  Code::new(),
         scope: Scope::new(),
      });

      closure(self);

      self.contexts.pop().expect(CODE_EXPECT)
   }

   fn emit_thunk(&mut self, span: Span, closure: impl FnOnce(&mut Self)) {
      let mut context = self.context(|this| closure(this));

      context.code.push_operation(span, Operation::Return);

      if !context.scope {
         self.push_constant(span, Value::Thunk());
         return;
      }

      self.push_constant(span, Constant::Blueprint(Arc::new(context.code)));
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

      self.push_constant(list.span(), Constant::Nil);
   }

   fn emit_attributes(&mut self, attributes: &node::Attributes) {
      if let Some(expression) = attributes.expression() {
         self.emit_thunk(attributes.span(), |this| {
            this.push_operation(expression.span(), Operation::Attributes);
            this.emit(expression);
         });
      } else {
         self.push_constant(
            attributes.span(),
            Constant::Attributes(Arc::new(IndexMap::with_hasher(FxHasher::default()))),
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
               this.push_constant(operation.span(), Constant::Nil);
            });
         },
      }
   }

   fn emit_path(&mut self, path: &node::Path) {
      self.emit_thunk(path.span(), |this| {
         let parts = path
            .parts()
            .filter(|part| !matches!(part, node::InterpolatedPartRef::Delimiter(_)))
            .collect::<Vec<_>>();

         if parts.len() != 1 {
            this.push_operation(path.span(), Operation::PathInterpolate);
            this.push_u64(parts.len() as _);
         }

         for part in parts {
            match part {
               node::InterpolatedPartRef::Content(content) => {
                  this.push_constant(content.span(), Constant::Path(content.text().to_owned()));
               },

               node::InterpolatedPartRef::Interpolation(interpolation) => {
                  this.push_operation(interpolation.span(), Operation::Force);
                  this.emit(interpolation.expression());
               },

               _ => {},
            }
         }
      });
   }

   fn emit_reference(&mut self, identifier: &node::Identifier) {}

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
            self.push_constant(rune.span(), Constant::Rune(rune.value()));
         },
         node::ExpressionRef::Integer(integer) => {
            self.push_constant(integer.span(), Constant::Integer(integer.value()));
         },
         node::ExpressionRef::Float(float) => {
            self.push_constant(float.span(), Constant::Float(float.value()));
         },

         node::ExpressionRef::If(_) => todo!(),
      }
   }
}

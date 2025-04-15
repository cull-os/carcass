use std::{
   ops,
   sync::Arc,
};

use cab_syntax::node;
use cab_why::{
   IntoSpan as _,
   Report,
   ReportSeverity,
   Span,
};

use crate::{
   Code,
   Constant,
   Operation,
};

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

      compiler.compile(node);

      Compile {
         code:    compiler
            .codes
            .pop()
            .expect("compiler must have at least one code at all times"),
         reports: compiler.reports,
      }
   }
}

struct Compiler {
   codes:   Vec<Code>,
   reports: Vec<Report>,
}

impl ops::Deref for Compiler {
   type Target = Code;

   fn deref(&self) -> &Self::Target {
      self
         .codes
         .last()
         .expect("compiler must have at least one code at all times")
   }
}

impl ops::DerefMut for Compiler {
   fn deref_mut(&mut self) -> &mut Self::Target {
      self
         .codes
         .last_mut()
         .expect("compiler must have at least one code at all times")
   }
}

impl Compiler {
   fn new() -> Self {
      Compiler {
         codes:   Vec::new(),
         reports: Vec::new(),
      }
   }

   fn code_new(&mut self) {
      self.codes.push(Code::new());
   }

   fn code_pop(&mut self) -> Code {
      self
         .codes
         .pop()
         .expect("compiler must have at least one code at all times")
   }

   fn emit_constant(&mut self, span: Span, constant: Constant) {
      let id = self.push_constant(constant);

      self.push_operation(span, Operation::Constant);
      self.push_u64(*id as u64);
   }

   fn compile_thunk(&mut self, span: Span, content: impl FnOnce(&mut Self)) {
      self.code_new();

      content(self);

      let mut code = self.code_pop();
      code.push_operation(span, Operation::Return);

      let blueprint_index = self.push_constant(Constant::Blueprint(Arc::new(code)));

      self.push_u64(*blueprint_index as u64);
   }

   fn compile_prefix_operation(&mut self, operation: &node::PrefixOperation) {
      self.compile(operation.right());
      self.push_operation(operation.right().span(), Operation::Force);

      self.push_operation(operation.span(), match operation.operator() {
         node::PrefixOperator::Swwallation => Operation::Swwallation,
         node::PrefixOperator::Negation => Operation::Negation,
         node::PrefixOperator::Not => Operation::Not,
         node::PrefixOperator::Try => Operation::Try,
      });
   }

   fn compile_infix_operation(&mut self, operation: &node::InfixOperation) {
      if operation.operator() != node::InfixOperator::Pipe {
         self.compile(operation.left());
         self.push_operation(operation.left().span(), Operation::Force);

         self.compile(operation.right());
         self.push_operation(operation.right().span(), Operation::Force);
      } else {
         self.compile(operation.right());
         self.push_operation(operation.right().span(), Operation::Force);

         self.compile(operation.left());
         self.push_operation(operation.left().span(), Operation::Force);
      }

      self.push_operation(operation.span(), match operation.operator() {
         node::InfixOperator::Same => Operation::Same,
         node::InfixOperator::Sequence => Operation::Sequence,

         node::InfixOperator::ImplicitApply
         | node::InfixOperator::Apply
         // Parameter order was swapped in the bytecode.
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
            self.push_operation(operation.span(), Operation::Equal);
            self.push_operation(operation.span(), Operation::Not);
            return;
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
      });
   }

   fn compile_suffix_operation(&mut self, operation: &node::SuffixOperation) {
      match operation.operator() {
         node::SuffixOperator::Same => self.compile(operation.left()),
         node::SuffixOperator::Sequence => {
            self.compile(operation.left());
            self.push_operation(operation.left().span(), Operation::Force);
            self.push_constant(Constant::Integer(0xDEADBEAFu32.into())); // TODO: Use a proper value.
         },
      }
   }

   fn compile(&mut self, expression: node::ExpressionRef<'_>) {
      match expression {
         node::ExpressionRef::Error(_) => unreachable!(),

         node::ExpressionRef::Parenthesis(parenthesis) => {
            self.compile(parenthesis.expression().expect("node must be validated"))
         },

         node::ExpressionRef::List(_list) => todo!(),
         node::ExpressionRef::Attributes(_attributes) => todo!(),

         node::ExpressionRef::PrefixOperation(prefix_operation) => {
            self.compile_thunk(prefix_operation.span(), |this| {
               this.compile_prefix_operation(prefix_operation);
            })
         },
         node::ExpressionRef::InfixOperation(infix_operation) => {
            self.compile_thunk(infix_operation.span(), |this| {
               this.compile_infix_operation(infix_operation);
            })
         },
         node::ExpressionRef::SuffixOperation(suffix_operation) => {
            self.compile_thunk(suffix_operation.span(), |this| {
               this.compile_suffix_operation(suffix_operation);
            })
         },

         node::ExpressionRef::Island(_island) => todo!(),
         node::ExpressionRef::Path(_path) => todo!(),
         node::ExpressionRef::Bind(_bind) => todo!(),
         node::ExpressionRef::Identifier(_identifier) => todo!(),
         node::ExpressionRef::SString(_sstring) => todo!(),

         node::ExpressionRef::Rune(rune) => {
            self.emit_constant(rune.span(), Constant::Rune(rune.value()))
         },
         node::ExpressionRef::Integer(integer) => {
            self.emit_constant(integer.span(), Constant::Integer(integer.value()))
         },
         node::ExpressionRef::Float(float) => {
            self.emit_constant(float.span(), Constant::Float(float.value()))
         },

         node::ExpressionRef::If(_) => todo!(),
      }
   }
}

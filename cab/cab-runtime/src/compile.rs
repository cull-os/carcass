use cab_syntax::node;
use cab_why::{
   IntoSpan,
   Report,
   ReportSeverity,
};

use crate::{
   Code,
   Operation,
   Value,
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

impl Compiler {
   fn new() -> Self {
      Compiler {
         codes:   Vec::new(),
         reports: Vec::new(),
      }
   }

   fn code(&mut self) -> &mut Code {
      self
         .codes
         .last_mut()
         .expect("compiler must have at least one code at all times")
   }

   fn emit_constant(&mut self, node: &impl IntoSpan, value: Value) {
      let id = self.code().push_constant(value);

      self.code().push_operation(node.span(), Operation::Constant);
      self.code().push_u64(*id as u64);
   }

   fn compile(&mut self, expression: node::ExpressionRef<'_>) {
      match expression {
         node::ExpressionRef::Error(_) => unreachable!(),
         node::ExpressionRef::Parenthesis(parenthesis) => {
            self.compile(parenthesis.expression().expect("node must be validated"))
         },
         node::ExpressionRef::List(_list) => todo!(),
         node::ExpressionRef::Attributes(_attributes) => todo!(),
         node::ExpressionRef::PrefixOperation(_prefix_operation) => todo!(),
         node::ExpressionRef::InfixOperation(_infix_operation) => todo!(),
         node::ExpressionRef::SuffixOperation(_suffix_operation) => todo!(),
         node::ExpressionRef::Island(_island) => todo!(),
         node::ExpressionRef::Path(_path) => todo!(),
         node::ExpressionRef::Bind(_bind) => todo!(),
         node::ExpressionRef::Identifier(_identifier) => todo!(),
         node::ExpressionRef::SString(_sstring) => todo!(),
         node::ExpressionRef::Rune(rune) => self.emit_constant(&**rune, Value::Rune(rune.value())),
         node::ExpressionRef::Integer(integer) => {
            self.emit_constant(&**integer, Value::Integer(integer.value()))
         },
         node::ExpressionRef::Float(float) => {
            self.emit_constant(&**float, Value::Float(float.value()))
         },
         node::ExpressionRef::If(_) => todo!(),
      }
   }
}

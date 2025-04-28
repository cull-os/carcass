use cab_syntax::node::{
   self,
   Parted,
};
use cab_why::{
   IntoSpan,
   Report,
};
use smallvec::{
   SmallVec,
   smallvec,
};

use super::Compiler;
use crate::{
   LocalName,
   LocalPosition,
   Scope,
};

enum Boolean<'a> {
   False(node::ExpressionRef<'a>),
   True(node::ExpressionRef<'a>),
   Other(node::ExpressionRef<'a>),
}

impl<'a> From<node::ExpressionRef<'a>> for Boolean<'a> {
   fn from(expression: node::ExpressionRef<'a>) -> Self {
      let node::ExpressionRef::Identifier(identifier) = expression else {
         return Self::Other(expression);
      };

      let content: SmallVec<&str, 4> = match identifier.value() {
         node::IdentifierValueRef::Plain(plain) => smallvec![plain.text()],

         node::IdentifierValueRef::Quoted(quoted) => {
            quoted
               .parts()
               .filter_map(|part| {
                  match part {
                     node::InterpolatedPartRef::Content(content) => Some(content.text()),

                     _ => None,
                  }
               })
               .collect()
         },
      };

      if content.len() != 1 {
         return Self::Other(expression);
      }

      match content[0] {
         "true" => Self::True(expression),
         "false" => Self::False(expression),
         _ => Self::Other(expression),
      }
   }
}

impl Compiler {
   fn optimize_infix_operation<'a>(
      &mut self,
      operation: &'a node::InfixOperation,
   ) -> node::ExpressionRef<'a> {
      use Boolean::*;
      use node::InfixOperator::{
         All,
         And,
         Any,
         Or,
      };

      // TODO: Inefficient as hell. Make it borrow data instead.
      let (LocalPosition::Undefined, LocalPosition::Undefined) = (
         Scope::locate(&self.scope, &LocalName::new(smallvec!["false".to_owned()])),
         Scope::locate(&self.scope, &LocalName::new(smallvec!["true".to_owned()])),
      ) else {
         return operation.into();
      };

      let operator_span = operation.operator_token().map(|token| token.span());

      match (
         operation.left().into(),
         operation.operator(),
         operation.right().into(),
      ) {
         // false ||, false |
         // true &&, true &
         (False(boolean), Or | Any, Other(expression))
         | (Other(expression), Or | Any, False(boolean))
         | (True(boolean), And | All, Other(expression))
         | (Other(expression), And | All, True(boolean)) => {
            self.reports.push(
               Report::warn("unnecessary infix operation")
                  .primary(boolean.span().cover(operator_span.unwrap()), "delete this"),
            );

            expression
         },

         // false &&, false &
         // true ||, true |
         (True(boolean), Or | Any, Other(_))
         | (Other(_), Or | Any, True(boolean))
         | (False(boolean), And | All, Other(_))
         | (Other(_), And | All, False(boolean)) => {
            self
               .reports
               .push(Report::warn("this expression never changes").primary(
                  boolean.span().cover(operator_span.unwrap()),
                  "because of this",
               ));

            operation.into()
         },

         _ => operation.into(),
      }
   }

   pub fn optimize<'a>(&mut self, expression: node::ExpressionRef<'a>) -> node::ExpressionRef<'a> {
      match expression {
         node::ExpressionRef::InfixOperation(operation) => self.optimize_infix_operation(operation),

         _ => expression,
      }
   }
}

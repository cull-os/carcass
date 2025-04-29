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

use super::{
   Compiler,
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

impl<'a> Compiler<'a> {
   fn optimize_infix_operation(
      &mut self,
      operation: &'a node::InfixOperation,
   ) -> node::ExpressionRef<'a> {
      use Boolean::{
         False,
         Other,
         True,
      };
      use node::InfixOperator::{
         All,
         And,
         Any,
         Or,
      };

      let real_false = !Scope::is_user_defined(&mut self.scopes, "false");
      let real_true = !Scope::is_user_defined(&mut self.scopes, "true");

      let operator_span = operation.operator_token().map(|token| token.span());

      match (
         operation.left().into(),
         operation.operator(),
         operation.right().into(),
      ) {
         // false ||, false |
         (False(boolean), Or | Any, Other(expression))
         | (Other(expression), Or | Any, False(boolean))
            if real_false =>
         {
            self.reports.push(
               Report::warn("this `false` has no effect on the result of the operation")
                  .primary(boolean.span().cover(operator_span.unwrap()), "delete this"),
            );

            expression
         },

         // false &&, false &
         (False(boolean), And | All, right) if real_false => {
            self.reports.push(
               Report::warn("this expression is always `false`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span.unwrap()),
                     "this might be unwanted",
                  ),
            );

            if let Other(expression) = right {
               self.emit_dead(expression);
            }

            boolean
         },

         // && false, & false
         (_, And | All, False(boolean)) if real_false => {
            self.reports.push(
               Report::warn("this expression is always `false`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span.unwrap()),
                     "this might be unwanted",
                  ),
            );

            operation.into()
         },

         // true &&, true &
         (True(boolean), And | All, Other(expression))
         | (Other(expression), And | All, True(boolean))
            if real_true =>
         {
            self.reports.push(
               Report::warn("this `true` has no effect on the result of the operation")
                  .primary(boolean.span().cover(operator_span.unwrap()), "delete this"),
            );

            expression
         },

         // true ||, true |
         (True(boolean), Or | Any, right) if real_true => {
            self.reports.push(
               Report::warn("this expression is always `true`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span.unwrap()),
                     "this might be unwanted",
                  ),
            );

            if let Other(expression) = right {
               self.emit_dead(expression);
            }

            boolean
         },

         // || true, | true
         (_, Or | Any, True(boolean)) if real_true => {
            self.reports.push(
               Report::warn("this expression is always `true`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span.unwrap()),
                     "this might be unwanted",
                  ),
            );

            operation.into()
         },

         _ => operation.into(),
      }
   }

   pub fn optimize(&mut self, expression: node::ExpressionRef<'a>) -> node::ExpressionRef<'a> {
      match expression {
         node::ExpressionRef::InfixOperation(operation) => self.optimize_infix_operation(operation),

         _ => expression,
      }
   }
}

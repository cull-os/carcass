use std::borrow::Cow;

use cab_span::{
   IntoSpan as _,
   Span,
};
use cab_syntax::{
   Segment,
   Segmented as _,
   node,
};
use smallvec::{
   SmallVec,
   smallvec,
};
use ust::report::Report;

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

      let content = match identifier.value() {
         node::IdentifierValueRef::Plain(plain) => smallvec![Cow::Borrowed(plain.text())],

         node::IdentifierValueRef::Quoted(quoted) => {
            quoted
               .segments()
               .into_iter()
               .filter_map(|segment| {
                  match segment {
                     Segment::Content { content, .. } => Some(Cow::Owned(content)),

                     Segment::Interpolation(_) => None,
                  }
               })
               .collect::<SmallVec<Cow<'_, str>, 4>>()
         },
      };

      if content.len() != 1 {
         return Self::Other(expression);
      }

      match content[0].as_ref() {
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

      let operator_span = match operation.operator_token() {
         Some(token) => token.span(),

         None => Span::at(operation.left().span().end, 0_u32),
      };

      match (
         Boolean::from(operation.left()),
         operation.operator(),
         Boolean::from(operation.right()),
      ) {
         // false ||, false |
         (False(boolean), Or | Any, Other(expression))
         | (Other(expression), Or | Any, False(boolean))
            if real_false =>
         {
            self.reports.push(
               Report::warn("this `false` has no effect on the operation")
                  .primary(boolean.span().cover(operator_span), "delete this"),
            );

            expression
         },

         // false &&, false &
         (False(boolean), And | All, right) if real_false => {
            self.reports.push(
               Report::warn("this expression is always `false`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span),
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
                     boolean.span().cover(operator_span),
                     "this might be unwanted",
                  ),
            );

            node::ExpressionRef::from(operation)
         },

         // true &&, true &
         (True(boolean), And | All, Other(expression))
         | (Other(expression), And | All, True(boolean))
            if real_true =>
         {
            self.reports.push(
               Report::warn("this `true` has no effect on the operation")
                  .primary(boolean.span().cover(operator_span), "delete this"),
            );

            expression
         },

         // true ||, true |
         (True(boolean), Or | Any, right) if real_true => {
            self.reports.push(
               Report::warn("this expression is always `true`")
                  .secondary(operation.span(), "this expression")
                  .primary(
                     boolean.span().cover(operator_span),
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
                     boolean.span().cover(operator_span),
                     "this might be unwanted",
                  ),
            );

            node::ExpressionRef::from(operation)
         },

         _ => node::ExpressionRef::from(operation),
      }
   }

   pub fn optimize(&mut self, expression: node::ExpressionRef<'a>) -> node::ExpressionRef<'a> {
      match expression {
         node::ExpressionRef::InfixOperation(operation) => self.optimize_infix_operation(operation),

         _ => expression,
      }
   }
}

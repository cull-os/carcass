use std::ops;

use cab_syntax::lode;
use cab_util::{
   into,
   suffix::Arc as _,
};
use dup::Dupe as _;
use ranged::{
   IntoSpan as _,
   Span,
   Spanned,
};
use smallvec::SmallVec;

use crate::{
   Code,
   Operation,
   Value,
   value,
};

const EXPECT_CODE: &str = "emitter must have at least one code at all times";

pub struct CompileOracle {
   _reserved: (),
}

#[bon::bon]
impl CompileOracle {
   #[must_use]
   pub fn new() -> Self {
      Self { _reserved: () }
   }

   #[expect(clippy::unused_self)]
   #[builder(finish_fn(name = "path"))]
   #[must_use]
   pub fn compile(
      &self,
      #[builder(start_fn)] expression: lode::Resolved<'_, &lode::Expression>,
      #[builder(finish_fn)] path: value::Path,
   ) -> Code {
      let mut emitter = Emitter::new(path);

      emitter.emit_scope(expression.span(), |this| {
         this.emit_force(expression);
      });

      emitter.codes.pop().expect(EXPECT_CODE)
   }
}

struct Emitter {
   codes: Vec<Code>,
}

impl ops::Deref for Emitter {
   type Target = Code;

   fn deref(&self) -> &Self::Target {
      self.codes.last().expect(EXPECT_CODE)
   }
}

impl ops::DerefMut for Emitter {
   fn deref_mut(&mut self) -> &mut Self::Target {
      self.codes.last_mut().expect(EXPECT_CODE)
   }
}

impl Emitter {
   fn new(path: value::Path) -> Self {
      Self {
         codes: vec![Code::new(path)],
      }
   }
}

#[bon::bon]
impl Emitter {
   fn emit_push(&mut self, span: Span, value: impl Into<Value>) {
      into!(value);

      let index = self.value(value);

      self.push_operation(span, Operation::Push);
      self.push_u64(*index as _);
   }

   fn emit_scope(&mut self, span: Span, with: impl FnOnce(&mut Self)) {
      self.push_operation(span, Operation::ScopeStart);
      with(self);
      self.push_operation(span, Operation::ScopeEnd);
   }

   fn emit_thunk_start(&mut self) {
      self.codes.push(Code::new(self.path().dupe()));
   }

   #[builder(finish_fn(name = "needs_argument"))]
   fn emit_thunkable_end(
      &mut self,
      #[builder(start_fn)] span: Span,
      #[builder(finish_fn)] needs_argument: bool,
   ) {
      let code = self.codes.pop().expect(EXPECT_CODE);

      self.emit_push(
         span,
         if needs_argument {
            Value::NeedsArgumentToThunk(code.arc())
         } else {
            Value::Thunkable(code.arc())
         },
      );
   }

   #[builder(finish_fn(name = "with"))]
   fn emit_thunk(
      &mut self,
      #[builder(start_fn)] span: Span,
      #[builder(finish_fn)] with: impl FnOnce(&mut Self),
      #[builder(default = true)] if_: bool,
      #[builder(default = false)] needs_argument: bool,
   ) {
      if !if_ {
         with(self);
         return;
      }

      self.emit_thunk_start();
      with(self);
      self.emit_thunkable_end(span).needs_argument(needs_argument);
   }

   fn emit_parenthesis<'arena>(
      &mut self,
      parenthesis: lode::Resolved<'arena, Spanned<&'arena lode::Parenthesis>>,
   ) {
      self.emit_scope(parenthesis.span(), |this| {
         this.emit(parenthesis.expression());
      });
   }

   fn emit_list<'arena>(&mut self, list: lode::Resolved<'arena, Spanned<&'arena lode::List>>) {
      for item in list.items() {
         self.emit_thunk_start();
         self.emit(item);
      }

      self.emit_push(list.span(), value::Nil);

      for item in list.items() {
         self.push_operation(list.span(), Operation::Construct);
         self.emit_thunkable_end(item.span()).needs_argument(false);
      }
   }

   fn emit_attributes<'arena>(
      &mut self,
      attributes: lode::Resolved<'arena, Spanned<&'arena lode::Attributes>>,
   ) {
      match attributes.expression() {
         Some(expression) => {
            self.emit_thunk(attributes.span()).with(|this| {
               this.emit_scope(attributes.span(), |this| {
                  this.emit_force(expression);
                  let to_end = {
                     this.push_operation(expression.span(), Operation::JumpIfError);
                     this.push_u16(u16::default())
                  };
                  this.push_operation(expression.span(), Operation::ScopePush);
                  this.push_operation(expression.span(), Operation::Swap);
                  this.push_operation(expression.span(), Operation::Pop);
                  this.point_here(to_end);
               });
            });
         },

         None => {
            self.emit_push(attributes.span(), value::attributes::new! {});
         },
      }
   }

   #[builder(finish_fn(name = "right"))]
   fn emit_select(
      &mut self,
      #[builder(start_fn)] span: Span,
      #[builder(finish_fn)] right: (Span, impl FnOnce(&mut Emitter)),
      left: (Span, impl FnOnce(&mut Emitter)),
   ) {
      let (right_span, emit_right) = right;
      let (left_span, emit_left) = left;

      emit_left(self);
      let to_end = {
         self.push_operation(left_span, Operation::JumpIfError);
         self.push_u16(u16::default())
      };

      self.push_operation(left_span, Operation::ScopeSwap);

      let to_end_ = {
         self.push_operation(span, Operation::JumpIfError);
         self.push_u16(u16::default())
      };

      emit_right(self);
      self.push_operation(right_span, Operation::Force);

      self.push_operation(span, Operation::Swap);
      self.push_operation(span, Operation::ScopeSwap);
      self.push_operation(span, Operation::Pop);

      self.point_here(to_end_);
      self.point_here(to_end);
   }

   fn emit_same<'arena>(&mut self, same: lode::Resolved<'arena, Spanned<&'arena lode::Same>>) {
      self.emit(same.left());
      self.emit(same.right());
      self.push_operation(same.span(), Operation::All);
   }

   fn emit_sequence<'arena>(
      &mut self,
      sequence: lode::Resolved<'arena, Spanned<&'arena lode::Sequence>>,
   ) {
      let left = sequence.left();
      let right = sequence.right();

      self.emit_force(left);
      let to_end = {
         self.push_operation(sequence.span(), Operation::JumpIfError);
         self.push_u16(u16::default())
      };
      self.push_operation(sequence.span(), Operation::Pop);

      self.emit_force(right);

      self.point_here(to_end);
   }

   fn emit_call<'arena>(&mut self, call: lode::Resolved<'arena, Spanned<&'arena lode::Call>>) {
      let function = call.function();
      let argument = call.argument();

      self.emit_force(function);

      let to_end = {
         self.push_operation(call.span(), Operation::JumpIfError);
         self.push_u16(u16::default())
      };

      self.emit(argument);
      self.push_operation(call.span(), Operation::Call);
      self.point_here(to_end);
   }

   fn emit_construct<'arena>(
      &mut self,
      construct: lode::Resolved<'arena, Spanned<&'arena lode::Construct>>,
   ) {
      self.emit(construct.head());
      self.emit(construct.tail());
      self.push_operation(construct.span(), Operation::Construct);
   }

   fn emit_select_expression<'arena>(
      &mut self,
      select: lode::Resolved<'arena, Spanned<&'arena lode::Select>>,
   ) {
      let scope = select.scope();
      let expression = select.expression();

      self
         .emit_select(select.span())
         .left((scope.span(), |this| this.emit_force(scope)))
         .right((expression.span(), |this| {
            this.emit_scope(expression.span(), |this| {
               this.emit(expression);
            });
         }));
   }

   fn emit_equal<'arena>(&mut self, equal: lode::Resolved<'arena, Spanned<&'arena lode::Equal>>) {
      self.emit(equal.left());
      self.emit(equal.right());
      self.push_operation(equal.span(), Operation::Equal);
   }

   fn emit_and<'arena>(&mut self, and: lode::Resolved<'arena, Spanned<&'arena lode::And>>) {
      let left = and.left();
      let right = and.right();

      self.emit_force(left);
      let to_right = {
         self.push_operation(left.span(), Operation::JumpIf);
         self.push_u16(u16::default())
      };
      let over_right = {
         self.push_operation(and.span(), Operation::Jump);
         self.push_u16(u16::default())
      };

      self.point_here(to_right);
      self.push_operation(and.span(), Operation::Pop);
      self.emit_force(right);
      self.push_operation(right.span(), Operation::AssertBoolean);

      self.point_here(over_right);
   }

   fn emit_or<'arena>(&mut self, or: lode::Resolved<'arena, Spanned<&'arena lode::Or>>) {
      let left = or.left();
      let right = or.right();

      self.emit_force(left);

      let to_end = {
         self.push_operation(or.span(), Operation::JumpIf);
         self.push_u16(u16::default())
      };

      self.push_operation(or.span(), Operation::Pop);
      self.emit_force(right);
      self.push_operation(or.span(), Operation::AssertBoolean);

      self.point_here(to_end);
   }

   fn emit_all<'arena>(&mut self, all: lode::Resolved<'arena, Spanned<&'arena lode::All>>) {
      self.emit(all.left());
      self.emit(all.right());
      self.push_operation(all.span(), Operation::All);
   }

   fn emit_any<'arena>(&mut self, any: lode::Resolved<'arena, Spanned<&'arena lode::Any>>) {
      self.emit(any.left());
      self.emit(any.right());
      self.push_operation(any.span(), Operation::Any);
   }

   fn emit_lambda<'arena>(
      &mut self,
      lambda: lode::Resolved<'arena, Spanned<&'arena lode::Lambda>>,
   ) {
      let argument = lambda.argument();
      let expression = lambda.expression();

      self
         .emit_thunk(lambda.span())
         .needs_argument(true)
         .with(|this| {
            this.emit_scope(lambda.span(), |this| {
               this.emit_force(argument);
               this.push_operation(argument.span(), Operation::Equal);

               let to_body = {
                  this.push_operation(argument.span(), Operation::JumpIf);
                  this.push_u16(u16::default())
               };

               this.push_operation(lambda.span(), Operation::Pop);
               this.emit_push(
                  argument.span(),
                  value::Error::new(value::string::new!("parameters were not equal")).arc(),
               );

               let over_body = {
                  this.push_operation(lambda.span(), Operation::Jump);
                  this.push_u16(u16::default())
               };

               this.point_here(to_body);
               this.push_operation(lambda.span(), Operation::Pop);
               this.emit_force(expression);

               this.point_here(over_body);
            });
         });
   }

   fn emit_path<'arena>(&mut self, path: lode::Resolved<'arena, Spanned<&'arena lode::Path>>) {
      let segments = path.segments();
      let segments_are_trivial = segments.is_trivial();

      self
         .emit_thunk(path.span())
         .if_(!segments_are_trivial)
         .with(|this| {
            let segments = segments.into_iter().collect::<SmallVec<_, 4>>();

            for segment in &segments {
               match &segment.value {
                  &lode::Segment::Content(ref content) => {
                     let content = &***content;

                     this.emit_push(
                        segment.span(),
                        value::Path::rootless(
                           content
                              .split(value::path::SEPARATOR)
                              .filter(|part| !part.is_empty())
                              .map(value::SString::from)
                              .collect(),
                        ),
                     );
                  },

                  &lode::Segment::Interpolation(ref interpolation) => {
                     this.emit_scope(segment.span(), |this| {
                        this.emit_force(*interpolation);
                     });
                  },
               }
            }

            if !segments_are_trivial {
               this.push_operation(path.span(), Operation::Interpolate);
               this.push_u64(segments.len() as _);
            }
         });
   }

   fn emit_identifier_like<'arena>(
      &mut self,
      segments: lode::Resolved<'arena, &'arena lode::Segments>,
      span: Span,
      is_bind: bool,
   ) {
      let segments_are_trivial = segments.is_trivial();
      let needs_thunk = !is_bind || !segments_are_trivial;

      self.emit_thunk(span).if_(needs_thunk).with(|this| {
         let segments = segments.into_iter().collect::<SmallVec<_, 4>>();

         for segment in &segments {
            match &segment.value {
               &lode::Segment::Content(ref content) => {
                  let content = &***content;

                  this.emit_push(
                     segment.span(),
                     if is_bind {
                        Value::Bind(value::SString::from(content))
                     } else {
                        Value::Reference(value::SString::from(content))
                     },
                  );
               },

               &lode::Segment::Interpolation(ref interpolation) => {
                  this.emit_scope(segment.span(), |this| {
                     this.emit_force(*interpolation);
                  });
               },
            }
         }

         if !segments_are_trivial {
            this.push_operation(span, Operation::Interpolate);
            this.push_u64(segments.len() as _);
         }

         if !is_bind {
            this.push_operation(span, Operation::Resolve);
         }
      });
   }

   fn emit_bind<'arena>(&mut self, bind: lode::Resolved<'arena, Spanned<&'arena lode::Bind>>) {
      self.emit_identifier_like(bind.segments(), bind.span(), true);
   }

   fn emit_identifier<'arena>(
      &mut self,
      identifier: lode::Resolved<'arena, Spanned<&'arena lode::Identifier>>,
   ) {
      self.emit_identifier_like(identifier.segments(), identifier.span(), false);
   }

   fn emit_string<'arena>(
      &mut self,
      string: lode::Resolved<'arena, Spanned<&'arena lode::SString>>,
   ) {
      let segments = string.segments();
      let segments_are_trivial = segments.is_trivial();

      self
         .emit_thunk(string.span())
         .if_(!segments_are_trivial)
         .with(|this| {
            let segments = segments.into_iter().collect::<SmallVec<_, 4>>();

            for segment in &segments {
               match &segment.value {
                  &lode::Segment::Content(ref content) => {
                     this.emit_push(segment.span(), value::SString::from(&***content));
                  },

                  &lode::Segment::Interpolation(ref interpolation) => {
                     this.emit_scope(segment.span(), |this| {
                        this.emit_force(*interpolation);
                     });
                  },
               }
            }

            if !segments_are_trivial {
               this.push_operation(string.span(), Operation::Interpolate);
               this.push_u64(segments.len() as _);
            }
         });
   }

   fn emit_if<'arena>(&mut self, if_: lode::Resolved<'arena, Spanned<&'arena lode::If>>) {
      let condition = if_.condition();
      let consequence = if_.consequence();
      let alternative = if_.alternative();

      self.emit_thunk(if_.span()).with(|this| {
         this.emit_force(condition);
         let to_end = {
            this.push_operation(if_.span(), Operation::JumpIfError);
            this.push_u16(u16::default())
         };
         let to_consequence = {
            this.push_operation(if_.span(), Operation::JumpIf);
            this.push_u16(u16::default())
         };
         let to_end_ = {
            this.push_operation(if_.span(), Operation::JumpIfError);
            this.push_u16(u16::default())
         };

         this.push_operation(if_.span(), Operation::Pop);
         this.emit_scope(alternative.span(), |this| {
            this.emit_force(alternative);
         });
         let over_consequence = {
            this.push_operation(if_.span(), Operation::Jump);
            this.push_u16(u16::default())
         };

         this.point_here(to_consequence);
         this.push_operation(if_.span(), Operation::Pop);
         this.emit_scope(consequence.span(), |this| {
            this.emit_force(consequence);
         });

         this.point_here(over_consequence);
         this.point_here(to_end);
         this.point_here(to_end_);
      });
   }

   #[stacksafe::stacksafe]
   fn emit<'arena>(&mut self, expression: lode::Resolved<'arena, &'arena lode::Expression>) {
      match expression.propagate() {
         lode::ExpressionPropagated::Parenthesis(parenthesis) => {
            self.emit_parenthesis(parenthesis);
         },

         lode::ExpressionPropagated::List(list) => {
            self.emit_list(list);
         },

         lode::ExpressionPropagated::Attributes(attributes) => {
            self.emit_attributes(attributes);
         },

         lode::ExpressionPropagated::Same(same) => {
            self.emit_same(same);
         },

         lode::ExpressionPropagated::Sequence(sequence) => {
            self.emit_sequence(sequence);
         },

         lode::ExpressionPropagated::Call(call) => {
            self.emit_call(call);
         },

         lode::ExpressionPropagated::Construct(construct) => {
            self.emit_construct(construct);
         },

         lode::ExpressionPropagated::Select(select) => {
            self.emit_select_expression(select);
         },

         lode::ExpressionPropagated::Equal(equal) => {
            self.emit_equal(equal);
         },

         lode::ExpressionPropagated::And(and) => {
            self.emit_and(and);
         },

         lode::ExpressionPropagated::Or(or) => {
            self.emit_or(or);
         },

         lode::ExpressionPropagated::All(all) => {
            self.emit_all(all);
         },

         lode::ExpressionPropagated::Any(any) => {
            self.emit_any(any);
         },

         lode::ExpressionPropagated::Lambda(lambda) => {
            self.emit_lambda(lambda);
         },

         lode::ExpressionPropagated::Path(path) => {
            self.emit_path(path);
         },

         lode::ExpressionPropagated::Bind(bind) => {
            self.emit_bind(bind);
         },

         lode::ExpressionPropagated::Identifier(identifier) => {
            self.emit_identifier(identifier);
         },

         lode::ExpressionPropagated::SString(string) => {
            self.emit_string(string);
         },

         lode::ExpressionPropagated::Char(char) => {
            self.emit_push(char.span(), ***char);
         },

         lode::ExpressionPropagated::Integer(integer) => {
            self.emit_push(integer.span(), value::Integer::from((**integer).to_owned()));
         },

         lode::ExpressionPropagated::Float(float) => {
            self.emit_push(float.span(), ***float);
         },

         lode::ExpressionPropagated::If(if_) => {
            self.emit_if(if_);
         },
      }
   }

   fn emit_force<'arena>(&mut self, expression: lode::Resolved<'arena, &'arena lode::Expression>) {
      self.emit(expression);
      self.push_operation(expression.span(), Operation::Force);
   }
}

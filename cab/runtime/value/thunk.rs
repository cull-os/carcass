#![allow(dead_code)]

use std::{
   mem,
   sync::Arc,
};

use dup::{
   Dupe,
   OptionDupedExt as _,
};
use tokio::sync::RwLock;

use crate::{
   Code,
   Location,
   Operation,
   Scopes,
   State,
   Value,
   value,
};

const EXPECT_SCOPE: &str = "must have at least once scope";

thread_local! {
   static BLACK_HOLE: ThunkInner = ThunkInner::SuspendedNative(Arc::new(||
      Value::error(value::string::new!("TODO better infinite recursion error"))
   ));

   static NOT_BOOLEAN: Value = Value::error(value::string::new!("TODO better assert boolean error"));

   static NOT_LAMBDA: Value = Value::error(value::string::new!("TODO better assert lambda error"));
}

#[derive(Clone, Dupe)]
enum ThunkInner {
   SuspendedNative(Arc<dyn Fn() -> Value + Send + Sync>),

   Suspended {
      location: Location,
      code:     Arc<Code>,
      argument: Option<Value>,
      scopes:   Scopes,
   },

   Evaluated(Value),
}

#[derive(Clone, Dupe)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

#[bon::bon]
impl Thunk {
   #[must_use]
   pub fn suspended_native(native: impl Fn() -> Value + Send + Sync + 'static) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::SuspendedNative(
         Arc::new(native),
      ))))
   }

   #[must_use]
   pub fn suspended(location: Location, code: Arc<Code>, scopes: Scopes) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::Suspended {
         location,
         argument: None,
         code,
         scopes,
      })))
   }

   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn lambda(
      #[builder(start_fn)] code: Arc<Code>,
      #[builder(start_fn)] scopes: Scopes,
      #[builder(finish_fn)] location: Location,
      argument: Value,
   ) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::Suspended {
         location,
         argument: Some(argument),
         code,
         scopes,
      })))
   }

   pub async fn evaluate(&self, state: &mut State) {
      let this = mem::replace(&mut *self.0.write().await, BLACK_HOLE.with(Dupe::dupe));

      let value = match this {
         ThunkInner::Evaluated(value) => value.dupe(),

         ThunkInner::SuspendedNative(native) => native(),

         #[expect(clippy::unneeded_field_pattern)]
         ThunkInner::Suspended {
            location: _,
            code,
            argument,
            mut scopes,
         } => {
            let mut stack = argument.into_iter().collect::<Vec<_>>();
            let items = &mut code.iter().peekable();

            while let Some((index, item)) = items.next() {
               let operation = *item.as_operation().expect("next item must be an operation");

               match operation {
                  Operation::Push => {
                     let value_index = items
                        .next()
                        .expect("push must not be the last item")
                        .1
                        .as_argument()
                        .expect("push must have an argument")
                        .as_value_index()
                        .expect("push argument must be a value index");

                     let value = &code[value_index];

                     stack.push(value.dupe());
                  },
                  Operation::Pop => {
                     stack
                        .pop()
                        .expect("pop operation must not be called on empty stack");
                  },
                  Operation::Swap => {
                     assert!(
                        stack.len() >= 2,
                        "swap must be called on stack of length 2 or higher",
                     );

                     let end = stack.len();
                     stack.swap(end, end - 1);
                  },
                  Operation::Jump => {
                     let target_index = items
                        .next()
                        .expect("jump must not be the last item")
                        .1
                        .as_argument()
                        .expect("jump must have an argument")
                        .as_byte_index()
                        .expect("jump argument must be a byte index");

                     let mut current_index = index;

                     // TODO: Off by one?
                     while current_index < target_index {
                        current_index = items.next().expect("jump must not jump out of bounds").0;
                     }
                  },
                  Operation::JumpIf => {
                     let target_index = items
                        .next()
                        .expect("jump-if must not be the last item")
                        .1
                        .as_argument()
                        .expect("jump-if must have an argument")
                        .as_byte_index()
                        .expect("jump-if argument must be a byte index");

                     let mut current_index = index;

                     let value = stack
                        .last_mut()
                        .expect("jump-if must be called on stack with at least one item");

                     let &mut Value::Boolean(value) = value else {
                        *value = NOT_BOOLEAN.with(Dupe::dupe);
                        continue;
                     };

                     if value {
                        // TODO: Off by one?
                        while current_index < target_index {
                           current_index =
                              items.next().expect("jump must not jump out of bounds").0;
                        }
                     }
                  },
                  Operation::Force => {
                     let value = stack
                        .last()
                        .expect("force must not be called on an empty stack");

                     let &Value::Thunk(ref thunk) = value else {
                        unreachable!("force must be called on a thunk")
                     };

                     Box::pin(thunk.dupe().evaluate(state)).await;
                  },
                  Operation::ScopeStart => {
                     scopes = scopes.push_front(value::attributes::new! {});
                  },
                  Operation::ScopeEnd => {
                     scopes
                        .drop_first()
                        .expect("scope-end must not be called with no scopes");
                  },
                  Operation::ScopePush => {
                     stack.push(Value::from(scopes.last().expect(EXPECT_SCOPE).dupe()));
                  },
                  Operation::ScopeSwap => {
                     let value = stack
                        .last_mut()
                        .expect("scope-swap must not be called on a empty stack");

                     let &mut Value::Attributes(ref mut value) = value else {
                        unreachable!("scope-swap must be called on an attributes");
                     };

                     let mut scope = scopes.first().expect(EXPECT_SCOPE).dupe();
                     mem::swap(&mut scope, value);

                     scopes = scopes.drop_first().expect(EXPECT_SCOPE).push_front(scope);
                  },
                  Operation::Interpolate => todo!(),
                  Operation::Resolve => {
                     let reference = stack
                        .last_mut()
                        .expect("resolve must not be called on an empty stack");

                     let &mut Value::Reference(ref identifier) = reference else {
                        unreachable!("resolve must be called on an identifier");
                     };

                     let value = scopes
                        .iter()
                        .find_map(|scope| scope.get(identifier))
                        .duped()
                        .unwrap_or_else(|| {
                           Value::error(value::SString::from(&*format!(
                              "TODO better undefined value message: '{identifier}'",
                              identifier = &**identifier,
                           )))
                        });

                     *reference = value;
                  },
                  Operation::AssertBoolean => {
                     let value = stack
                        .last_mut()
                        .expect("assert-boolean must not be called on an empty stack");

                     let &mut Value::Boolean(_) = value else {
                        *value = NOT_BOOLEAN.with(Dupe::dupe);
                        continue;
                     };
                  },
                  Operation::Swwallation => todo!(),
                  Operation::Negation => todo!(),
                  Operation::Not => todo!(),
                  Operation::Concat => todo!(),
                  Operation::Construct => todo!(),
                  Operation::Call => {
                     let argument = stack.pop().expect("call must not be called on empty stack");

                     let lambda_code = stack.pop().expect("call must not be called on empty stack");
                     let Value::Lambda(lambda_code) = lambda_code else {
                        stack.push(NOT_LAMBDA.with(Dupe::dupe));
                        continue;
                     };

                     let thunk = Self::lambda(lambda_code, scopes.dupe())
                        .argument(argument)
                        .location(code.read_operation(index).0);

                     stack.push(Value::from(thunk));
                  },
                  Operation::Update => todo!(),
                  Operation::LessOrEqual => todo!(),
                  Operation::Less => todo!(),
                  Operation::MoreOrEqual => todo!(),
                  Operation::More => todo!(),
                  Operation::Equal => todo!(),
                  Operation::All => todo!(),
                  Operation::Any => todo!(),
                  Operation::Addition => todo!(),
                  Operation::Subtraction => todo!(),
                  Operation::Multiplication => todo!(),
                  Operation::Power => todo!(),
                  Operation::Division => todo!(),
               }
            }

            let &[ref result] = &*stack else {
               unreachable!("stack must have exactly one item left");
            };

            result.dupe()
         },
      };

      *self.0.write().await = ThunkInner::Evaluated(value);
   }
}

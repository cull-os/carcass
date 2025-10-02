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
   State,
   Value,
   value,
};

thread_local! {
   static BLACK_HOLE: ThunkInner = ThunkInner::SuspendedNative(Arc::new(||
      Value::error(value::string::new!("infinite recursion"))
   ));
}

#[derive(Clone, Dupe)]
enum ThunkInner {
   SuspendedNative(Arc<dyn Fn() -> Value + Send + Sync>),

   Suspended {
      location: Location,
      code:     Arc<Code>,
      locals:   value::Attributes,
   },

   Evaluated(Value),
}

#[derive(Clone, Dupe)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

impl Thunk {
   #[must_use]
   pub fn suspended_native(native: impl Fn() -> Value + Send + Sync + 'static) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::SuspendedNative(
         Arc::new(native),
      ))))
   }

   #[must_use]
   pub fn suspended(location: Location, code: Arc<Code>) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::Suspended {
         location,
         code,
         locals: value::attributes::new! {},
      })))
   }

   pub async fn evaluate(&mut self, _state: &mut State) {
      let this = mem::replace(&mut *self.0.write().await, BLACK_HOLE.with(Dupe::dupe));

      let value = match this {
         ThunkInner::Evaluated(value) => value.dupe(),

         ThunkInner::SuspendedNative(native) => native(),

         ThunkInner::Suspended { code, .. } => {
            let mut stack = Vec::<Value>::new();
            let mut scope = value::attributes::new! {};

            let items = &mut code.iter().peekable();

            while let Some((_index, item)) = items.next() {
               let operation = *item
                  .as_operation()
                  .expect("next code item must be an operation");

               match operation {
                  Operation::Push => {
                     let value_index = items
                        .next()
                        .expect("push must not be the last byte")
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
                        .expect("pop operation must be called on non-empty stack");
                  },
                  Operation::Swap => {
                     assert!(
                        stack.len() >= 2,
                        "swap must be called on stack of length 2 or higher",
                     );

                     let end = stack.len();
                     stack.swap(end, end - 1);
                  },
                  Operation::Jump => todo!(),
                  Operation::JumpIf => todo!(),
                  Operation::Force => todo!(),
                  Operation::ScopeStart => todo!(),
                  Operation::ScopeEnd => todo!(),
                  Operation::ScopePush => todo!(),
                  Operation::ScopeSwap => {
                     let value = stack
                        .last_mut()
                        .expect("scope-swap must not be called on a nonempty stack");

                     let &mut Value::Attributes(ref mut value) = value else {
                        unreachable!("scope-swap must be called on an attributes");
                     };

                     mem::swap(&mut scope, value);
                  },
                  Operation::Interpolate => todo!(),
                  Operation::Resolve => {
                     let reference = stack
                        .last_mut()
                        .expect("resolve must be called on an on-empty stack");

                     let &mut Value::Reference(ref identifier) = reference else {
                        unreachable!("resolve must be called on an identifier");
                     };

                     let value = scope.get(identifier).duped().unwrap_or_else(|| {
                        Value::error(value::SString::from(&*format!(
                           "undefined value: '{identifier}'",
                           identifier = &**identifier,
                        )))
                     });

                     *reference = value;
                  },
                  Operation::AssertBoolean => todo!(),
                  Operation::Swwallation => todo!(),
                  Operation::Negation => todo!(),
                  Operation::Not => todo!(),
                  Operation::Concat => todo!(),
                  Operation::Construct => todo!(),
                  Operation::Call => todo!(),
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

            todo!("foo");
         },
      };

      *self.0.write().await = ThunkInner::Evaluated(value);
   }
}

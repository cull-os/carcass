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
   Operation,
   Scopes,
   State,
   Value,
   value,
};

const EXPECT_SCOPE: &str = "must have at least once scope";

#[derive(Clone, Dupe)]
enum ThunkInner {
   SuspendedNative {
      location: value::Location,
      code:     Arc<dyn Fn() -> Value + Send + Sync>,
      argument: Option<Value>,
   },

   Suspended {
      location: value::Location,
      code:     Arc<Code>,
      argument: Option<Value>,
      scopes:   Scopes,
   },

   Evaluated {
      scopagate: Option<Scopes>,
      value:     Value,
   },
}

impl ThunkInner {
   thread_local! {
      static NOT_BOOLEAN: value::Error = value::Error::new(value::string::new!("expected boolean, got something else"));

      static NOT_LAMBDA: value::Error = value::Error::new(value::string::new!("expected lambda, got something else"));

      static NOT_ATTRIBUTES: value::Error = value::Error::new(value::string::new!("expected attributes, got something else"));
   }

   fn black_hole(location: value::Location) -> Self {
      ThunkInner::SuspendedNative {
         location,
         code: Arc::new(|| {
            Value::from(Arc::new(value::Error::new(value::string::new!(
               "infinite recursion encountered"
            ))))
         }),
         argument: None,
      }
   }
}

#[derive(Clone, Dupe)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

#[bon::bon]
impl Thunk {
   #[must_use]
   pub fn suspended_native(
      location: value::Location,
      code: impl Fn() -> Value + Send + Sync + 'static,
   ) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::SuspendedNative {
         location,
         code: Arc::new(code),
         argument: None,
      })))
   }

   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn suspended(
      #[builder(start_fn)] code: Arc<Code>,
      #[builder(finish_fn)] location: value::Location,
      scopes: Scopes,
   ) -> Self {
      Self(Arc::new(RwLock::new(ThunkInner::Suspended {
         location,
         code,
         argument: None,
         scopes,
      })))
   }

   pub async fn argument(&self, arg: Value) -> Self {
      let mut inner = self.0.read().await.dupe();

      match inner {
         ThunkInner::SuspendedNative {
            ref mut argument, ..
         } => *argument = Some(arg),
         ThunkInner::Suspended {
            ref mut argument, ..
         } => *argument = Some(arg),
         ThunkInner::Evaluated { .. } => panic!("cannot add argument to evaluated thunk"),
      }

      Thunk(Arc::new(RwLock::new(inner)))
   }

   pub async fn get(&self) -> Option<(Option<Scopes>, Value)> {
      if let ThunkInner::Evaluated {
         ref scopagate,
         ref value,
      } = *self.0.read().await
      {
         Some((scopagate.dupe(), value.dupe()))
      } else {
         None
      }
   }

   pub async fn force(&self, state: &State) {
      let this = mem::replace(&mut *self.0.write().await, ThunkInner::Evaluated {
         scopagate: None,
         value:     Value::Nil(value::Nil),
      });

      let new = match this {
         evaluated @ ThunkInner::Evaluated { .. } => evaluated.dupe(),

         ThunkInner::SuspendedNative {
            location,
            code,
            argument: _argument,
         } => {
            *self.0.write().await = ThunkInner::black_hole(location);

            ThunkInner::Evaluated {
               scopagate: None,
               value:     code(),
            }
         },

         ThunkInner::Suspended {
            location,
            code,
            argument,
            mut scopes,
         } => {
            *self.0.write().await = ThunkInner::black_hole(location.dupe());

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

                     stack.push(match &code[value_index] {
                        &Value::Code { ref code, .. } => {
                           Value::from(
                              Thunk::suspended(code.dupe())
                                 .scopes(scopes.dupe())
                                 // FIXME: .location(code.read_operation(index).0),
                                 .location(location.dupe()),
                           )
                        },

                        other => other.dupe(),
                     });
                  },
                  Operation::Pop => {
                     stack
                        .pop()
                        .expect("pop operation must not be called on empty stack");
                  },
                  Operation::Swap => {
                     let &mut [.., ref mut x, ref mut y] = &mut *stack else {
                        unreachable!("swap must be called on stack of length 2 or higher");
                     };

                     mem::swap(x, y);
                  },
                  operation @ (Operation::Jump | Operation::JumpIf | Operation::JumpIfError) => {
                     let target_index = items
                        .next()
                        .expect("jump must not be the last item")
                        .1
                        .as_argument()
                        .expect("jump must have an argument")
                        .as_byte_index()
                        .expect("jump argument must be a byte index");

                     match operation {
                        Operation::Jump => {},
                        Operation::JumpIf => {
                           let value = stack.last_mut().expect(
                              "jump-if and jump-if-error must be called on stack with at least \
                               one item",
                           );

                           let &mut Value::Boolean(value) = value else {
                              *value = Value::from(Arc::from(
                                 ThunkInner::NOT_BOOLEAN
                                    .with(Dupe::dupe)
                                    .append_trace(code.read_operation(index).0),
                              ));
                              continue;
                           };

                           if !value {
                              continue;
                           }
                        },
                        Operation::JumpIfError => {
                           let value = stack.last_mut().expect(
                              "jump-if and jump-if-error must be called on stack with at least \
                               one item",
                           );

                           let &mut Value::Error(ref error) = value else {
                              continue;
                           };

                           *value = Value::from(Arc::new(
                              error.append_trace(code.read_operation(index).0),
                           ));
                        },
                        _ => unreachable!(),
                     }

                     while items
                        .next_if(|&(next_index, _)| next_index != target_index)
                        .is_some()
                     {}
                  },
                  Operation::Force => {
                     let mut value = stack
                        .pop()
                        .expect("force must not be called on an empty stack");

                     while let Value::Thunk(thunk) = value {
                        Box::pin(thunk.force(state)).await;

                        let (scope_new, value_new) = thunk
                           .get()
                           .await
                           .expect("thunk must contain value after forcing");

                        value = value_new;
                        scopes = scope_new.unwrap_or(scopes);
                     }

                     stack.push(value);
                  },
                  Operation::ScopeStart => {
                     scopes = scopes.push_front(value::attributes::new! {});
                  },
                  Operation::ScopeEnd => {
                     scopes = scopes
                        .drop_first()
                        .expect("scope-end must not be called with no scopes");
                  },
                  Operation::ScopePush => {
                     stack.push(Value::from(scopes.first().expect(EXPECT_SCOPE).dupe()));
                  },
                  Operation::ScopeSwap => {
                     let value = stack
                        .last_mut()
                        .expect("scope-swap must not be called on a empty stack");

                     let &mut Value::Attributes(ref mut value) = value else {
                        *value = Value::from(Arc::from(
                           ThunkInner::NOT_ATTRIBUTES
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0),
                        ));
                        continue;
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
                           Value::from(Arc::new(
                              value::Error::new(value::SString::from(&*format!(
                                 "undefined value: '{identifier}'",
                                 identifier = &**identifier,
                              )))
                              .append_trace(code.read_operation(index).0),
                           ))
                        });

                     *reference = value;
                  },
                  Operation::AssertBoolean => {
                     let value = stack
                        .last_mut()
                        .expect("assert-boolean must not be called on an empty stack");

                     let &mut Value::Boolean(_) = value else {
                        *value = Value::from(Arc::from(
                           ThunkInner::NOT_BOOLEAN
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0),
                        ));
                        continue;
                     };
                  },
                  Operation::Construct => {
                     let tail = stack
                        .pop()
                        .expect("construct must be called on a stack with 2 items or more");
                     let head = stack
                        .pop()
                        .expect("construct must be called on a stack with 2 items or more");

                     stack.push(Value::from(Arc::new(value::Cons(head, tail))));
                  },
                  Operation::Call => {
                     let argument = stack.pop().expect("call must not be called on empty stack");

                     let Value::Thunk(thunk) =
                        stack.pop().expect("call must not be called on empty stack")
                     else {
                        stack.push(Value::from(Arc::from(
                           ThunkInner::NOT_LAMBDA
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0),
                        )));
                        continue;
                     };

                     let thunk = thunk.argument(argument).await;

                     stack.push(Value::from(thunk));
                  },
                  Operation::Equal => {
                     let right = stack
                        .pop()
                        .expect("equal must be called on a stack with 2 items or more");
                     let left = stack
                        .pop()
                        .expect("equal must be called on a stack with 2 items or more");

                     // TODO: Not sure about the design here.
                     let (equal, binds) = Value::equals(&left, &right);

                     stack.push(Value::from(equal));
                     scopes = scopes
                        .drop_first()
                        .expect("equal must be called with a scope")
                        .push_front(
                           scopes
                              .first()
                              .expect("equal must be called with a scope")
                              .merge(&binds),
                        );
                  },
                  Operation::All => todo!(),
                  Operation::Any => todo!(),
               }
            }

            let len = stack.len();
            let Ok([value]) = <[_; 1]>::try_from(stack) else {
               unreachable!("stack must have exactly one item left, has {len}");
            };

            ThunkInner::Evaluated {
               scopagate: Some(scopes),
               value,
            }
         },
      };

      *self.0.write().await = new;
   }
}

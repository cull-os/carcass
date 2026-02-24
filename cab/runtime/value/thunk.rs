#![allow(dead_code)]

use std::{
   mem,
   sync::Arc,
};

use cab_util::{
   collect_vec,
   suffix::Arc as _,
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
   NeedsArgumentNative {
      location: value::Location,
      code:     Arc<dyn Fn() -> Value + Send + Sync>,
   },

   NeedsArgument {
      location: value::Location,
      code:     Arc<Code>,
      scopes:   Scopes,
   },

   ForceableNative {
      location: value::Location,
      code:     Arc<dyn Fn() -> Value + Send + Sync>,
      stack:    Option<Value>,
   },

   Forceable {
      location: value::Location,
      code:     Arc<Code>,
      stack:    Option<Value>,
      scopes:   Scopes,
   },

   Evaluated {
      scopagate: Option<Scopes>,
      value:     Value,
   },
}

impl ThunkInner {
   thread_local! {
      static NOT_BOOLEAN: Arc<value::Error> = value::Error::new(value::string::new!("expected boolean, got something else")).arc();

      static NOT_LAMBDA: Arc<value::Error> = value::Error::new(value::string::new!("expected lambda, got something else")).arc();

      static NOT_ATTRIBUTES: Arc<value::Error> = value::Error::new(value::string::new!("expected attributes, got something else")).arc();

      static INFINITE_RECURSION: Arc<value::Error> = value::Error::new(value::string::new!("infinite recursion encountered")).arc();
   }

   fn black_hole(location: value::Location) -> Self {
      ThunkInner::ForceableNative {
         location,
         code: (|| {
            Value::from(
               value::Error::new(value::string::new!("infinite recursion encountered")).arc(),
            )
         })
         .arc(),
         stack: None,
      }
   }
}

#[derive(Clone, Dupe)]
pub struct Thunk(Arc<RwLock<ThunkInner>>);

#[bon::bon]
impl Thunk {
   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn needs_argument_native(
      #[builder(start_fn)] code: impl Fn() -> Value + Send + Sync + 'static,
      #[builder(finish_fn)] location: value::Location,
   ) -> Self {
      Self(
         RwLock::new(ThunkInner::NeedsArgumentNative {
            location,
            code: code.arc(),
         })
         .arc(),
      )
   }

   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn needs_argument(
      #[builder(start_fn)] code: Arc<Code>,
      #[builder(finish_fn)] location: value::Location,
      scopes: Scopes,
   ) -> Self {
      Self(
         RwLock::new(ThunkInner::NeedsArgument {
            location,
            code,
            scopes,
         })
         .arc(),
      )
   }

   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn forceable_native(
      #[builder(start_fn)] code: impl Fn() -> Value + Send + Sync + 'static,
      #[builder(finish_fn)] location: value::Location,
   ) -> Self {
      Self(
         RwLock::new(ThunkInner::ForceableNative {
            location,
            code: code.arc(),
            stack: None,
         })
         .arc(),
      )
   }

   #[must_use]
   #[builder(finish_fn(name = "location"))]
   pub fn forceable(
      #[builder(start_fn)] code: Arc<Code>,
      #[builder(finish_fn)] location: value::Location,
      scopes: Scopes,
   ) -> Self {
      Self(
         RwLock::new(ThunkInner::Forceable {
            location,
            code,
            stack: None,
            scopes,
         })
         .arc(),
      )
   }

   pub async fn argument(&self, argument: Value) -> Option<Self> {
      let inner = self.0.read().await.dupe();

      let ThunkInner::NeedsArgument {
         location,
         code,
         scopes,
      } = inner
      else {
         return None;
      };

      Some(Thunk(
         RwLock::new(ThunkInner::Forceable {
            location,
            code,
            stack: Some(argument),
            scopes,
         })
         .arc(),
      ))
   }

   pub async fn get(&self) -> (Option<Scopes>, Value) {
      if let ThunkInner::Evaluated {
         ref scopagate,
         ref value,
      } = *self.0.read().await
      {
         (scopagate.dupe(), value.dupe())
      } else {
         (None, Value::from(self.dupe()))
      }
   }

   pub async fn is_whnf(&self) -> bool {
      matches!(
         *self.0.read().await,
         ThunkInner::Evaluated { .. }
            | ThunkInner::NeedsArgumentNative { .. }
            | ThunkInner::NeedsArgument { .. }
      )
   }

   pub async fn force(&self, state: &State) {
      let this = mem::replace(&mut *self.0.write().await, ThunkInner::Evaluated {
         scopagate: None,
         value:     Value::from(ThunkInner::INFINITE_RECURSION.with(Dupe::dupe)),
      });

      let new = match this {
         // WHNF? Only real typemasterbaiters will get this.
         whnf @ (ThunkInner::Evaluated { .. }
         | ThunkInner::NeedsArgumentNative { .. }
         | ThunkInner::NeedsArgument { .. }) => whnf.dupe(),

         ThunkInner::ForceableNative {
            location,
            code,
            // TODO
            stack: _argument,
         } => {
            *self.0.write().await = ThunkInner::black_hole(location);

            ThunkInner::Evaluated {
               scopagate: None,
               value:     code(),
            }
         },

         ThunkInner::Forceable {
            location,
            code,
            stack,
            mut scopes,
         } => {
            *self.0.write().await = ThunkInner::black_hole(location.dupe());

            collect_vec!(mut stack);

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
                        &Value::NeedsArgumentToThunk(ref thunk_code) => {
                           Value::from(
                              Thunk::needs_argument(thunk_code.dupe())
                                 .scopes(scopes.dupe())
                                 .location(code.read_operation(index).0),
                           )
                        },
                        &Value::Thunkable(ref thunk_code) => {
                           Value::from(
                              Thunk::forceable(thunk_code.dupe())
                                 .scopes(scopes.dupe())
                                 .location(code.read_operation(index).0),
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
                              *value = Value::from(
                                 ThunkInner::NOT_BOOLEAN
                                    .with(Dupe::dupe)
                                    .append_trace(code.read_operation(index).0)
                                    .arc(),
                              );
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

                           *value =
                              Value::from(error.append_trace(code.read_operation(index).0).arc());
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

                     while let Value::Thunk(ref thunk) = value
                        && !thunk.is_whnf().await
                     {
                        Box::pin(thunk.force(state)).await;

                        let (scope_new, value_new) = thunk.get().await;

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
                        *value = Value::from(
                           ThunkInner::NOT_ATTRIBUTES
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0)
                              .arc(),
                        );
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
                           Value::from(
                              value::Error::new(value::SString::from(&*format!(
                                 "undefined value: '{identifier}'",
                                 identifier = &**identifier,
                              )))
                              .append_trace(code.read_operation(index).0)
                              .arc(),
                           )
                        });

                     *reference = value;
                  },
                  Operation::AssertBoolean => {
                     let value = stack
                        .last_mut()
                        .expect("assert-boolean must not be called on an empty stack");

                     let &mut Value::Boolean(_) = value else {
                        *value = Value::from(
                           ThunkInner::NOT_BOOLEAN
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0)
                              .arc(),
                        );
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

                     stack.push(Value::from(value::Cons(head, tail).arc()));
                  },
                  Operation::Call => {
                     let argument = stack.pop().expect("call must not be called on empty stack");

                     let Value::Thunk(thunk) =
                        stack.pop().expect("call must not be called on empty stack")
                     else {
                        stack.push(Value::from(
                           ThunkInner::NOT_LAMBDA
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0)
                              .arc(),
                        ));
                        continue;
                     };

                     let Some(thunk) = thunk.argument(argument).await else {
                        stack.push(Value::from(
                           ThunkInner::NOT_LAMBDA
                              .with(Dupe::dupe)
                              .append_trace(code.read_operation(index).0)
                              .arc(),
                        ));
                        continue;
                     };

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

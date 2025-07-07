//! Error handling utilities.

#![feature(gen_blocks, if_let_guard, let_chains, trait_alias, try_trait_v2)]

use std::{
   fmt::{
      self,
      Write as _,
   },
   ops,
   process,
   result,
};

use dup::Dupe;
use ust::{
   Display,
   Write as _,
   style::StyledExt as _,
   terminal::{
      self,
      tag,
   },
};

/// Creates a [`Chain`] from the provided string literals.
///
/// # Example
///
/// ```rs
/// fn get_result() -> Result<()> {
///   Err(chain!("can't get the result"))
/// }
/// ```
#[macro_export]
macro_rules! chain {
   ($($t:tt)*) => {
      $crate::Chain::new().push_front_display(format!($($t)*))
   };
}

/// A macro that boils down to:
///
/// ```rs
/// return Err(chain!(arguments));
/// ```
#[macro_export]
macro_rules! bail {
   ($($t:tt)*) => {{
      Err($crate::chain!($($t)*))?;
      unreachable!()
   }};
}

/// A type alias for concise use of [`Chain`] with [`Result`](result::Result).
pub type Result<T> = result::Result<T, Chain>;

pub trait StdDisplay = fmt::Display + Send + Sync + 'static;

enum Link {
   StdDisplay(Box<dyn StdDisplay>),
   Tags(tag::Tags<'static>),
}

/// A chain.
#[derive(Clone, Dupe)]
pub struct Chain(rpds::ListSync<Link>);

impl Display for Chain {
   fn display_styled(&self, writer: &mut dyn ust::Write) -> fmt::Result {
      let reverse = self.0.reverse();
      let mut chain = reverse.iter().peekable();
      while let Some(link) = chain.next() {
         terminal::indent!(
            writer,
            header = if chain.peek().is_none() {
               "error:"
            } else {
               "cause:"
            }
            .red()
            .bold(),
         );

         #[expect(clippy::pattern_type_mismatch)]
         match link {
            Link::StdDisplay(display) => {
               let string = display.to_string();
               let mut chars = string.char_indices();

               if let Some((_, first)) = chars.next()
                  && let Some((second_start, second)) = chars.next()
                  && second.is_lowercase()
               {
                  writeln!(
                     writer,
                     "{first_lowercase}{rest}",
                     first_lowercase = first.to_lowercase(),
                     rest = &string[second_start..],
                  )?;
               } else {
                  writeln!(writer, "{string}")?;
               }
            },

            Link::Tags(tags) => {
               tags.display_styled(writer)?;
               writeln!(writer)?;
            },
         }
      }

      Ok(())
   }
}

impl Chain {
   #[must_use]
   pub fn new() -> Self {
      Self(rpds::List::new_sync())
   }

   #[must_use]
   pub fn push_front_display(&self, display: impl StdDisplay) -> Self {
      Self(self.0.push_front(Link::StdDisplay(Box::new(display))))
   }

   #[must_use]
   pub fn push_front_tags(&self, display: &impl tag::DisplayTags) -> Self {
      Self(
         self
            .0
            .push_front(Link::Tags(tag::Tags::from(display).into_owned())),
      )
   }
}

pub trait OptionExt<T> {
   fn ok_or_chain(self, display: impl StdDisplay) -> Result<T>;

   fn ok_or_chain_with<D: StdDisplay>(self, display: impl FnOnce() -> D) -> Result<T>;

   fn ok_or_tag(self, tags: &impl tag::DisplayTags) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
   fn ok_or_chain(self, display: impl StdDisplay) -> Result<T> {
      self.ok_or_else(|| Chain::new().push_front_display(display))
   }

   fn ok_or_chain_with<D: StdDisplay>(self, display: impl FnOnce() -> D) -> Result<T> {
      self.ok_or_else(|| Chain::new().push_front_display(display()))
   }

   fn ok_or_tag(self, tags: &impl tag::DisplayTags) -> Result<T> {
      self.ok_or_else(|| Chain::new().push_front_tags(tags))
   }
}

pub trait ResultExt<T> {
   fn chain_err(self, display: impl StdDisplay) -> Result<T>;

   fn chain_err_with<D: StdDisplay>(self, display: impl FnOnce() -> D) -> Result<T>;

   fn tag_err(self, tags: &impl tag::DisplayTags) -> Result<T>;
}

impl<T, E: StdDisplay> ResultExt<T> for result::Result<T, E> {
   fn chain_err(self, display: impl StdDisplay) -> Result<T> {
      self.map_err(|exist| {
         Chain::new()
            .push_front_display(exist)
            .push_front_display(display)
      })
   }

   fn chain_err_with<D: StdDisplay>(self, display: impl FnOnce() -> D) -> Result<T> {
      self.map_err(|exist| {
         Chain::new()
            .push_front_display(exist)
            .push_front_display(display())
      })
   }

   fn tag_err(self, tags: &impl tag::DisplayTags) -> Result<T> {
      self.map_err(|exist| Chain::new().push_front_display(exist).push_front_tags(tags))
   }
}

impl<T> ResultExt<T> for result::Result<T, Chain> {
   fn chain_err(self, display: impl StdDisplay) -> Result<T> {
      self.map_err(|chain| chain.push_front_display(display))
   }

   fn chain_err_with<D: StdDisplay>(self, display: impl FnOnce() -> D) -> Result<T> {
      self.map_err(|chain| chain.push_front_display(display()))
   }

   fn tag_err(self, tags: &impl tag::DisplayTags) -> Result<T> {
      self.map_err(|chain| chain.push_front_tags(tags))
   }
}

/// The termination type. Meant to be used as the return type of the main
/// function.
///
/// Can be created directly or from a [`Chain`] with the `?` operator. Will
/// pretty print the chain.
#[derive(Clone, Dupe)]
pub struct Termination(result::Result<(), Chain>);

impl ops::Try for Termination {
   type Output = ();
   type Residual = Self;

   fn from_output((): Self::Output) -> Self {
      Self::success()
   }

   fn branch(self) -> ops::ControlFlow<Self::Residual, Self::Output> {
      match self.0 {
         Ok(()) => ops::ControlFlow::Continue(()),
         Err(_) => ops::ControlFlow::Break(self),
      }
   }
}

impl<T, E: StdDisplay> ops::FromResidual<result::Result<T, E>> for Termination {
   fn from_residual(result: result::Result<T, E>) -> Self {
      match result {
         Ok(_) => Self::success(),
         Err(display) => Self::error(Chain::new().push_front_display(display)),
      }
   }
}

impl<T> ops::FromResidual<result::Result<T, Chain>> for Termination {
   fn from_residual(result: result::Result<T, Chain>) -> Self {
      match result {
         Ok(_) => Self::success(),
         Err(chain) => Self::error(chain),
      }
   }
}

impl ops::FromResidual<Termination> for Termination {
   fn from_residual(residual: Termination) -> Self {
      residual
   }
}

impl process::Termination for Termination {
   fn report(self) -> process::ExitCode {
      match self.0 {
         Ok(()) => process::ExitCode::SUCCESS,

         Err(chain) => {
            let writer = &mut terminal::stderr();
            let _ = chain.display_styled(writer);
            let _ = writer.finish();
            process::ExitCode::FAILURE
         },
      }
   }
}

impl Termination {
   /// Creates a successful [`Termination`] that returns success.
   #[must_use]
   pub fn success() -> Self {
      Self(Ok(()))
   }

   /// Creates a [`Termination`] from the provided [`Chain`].
   #[must_use]
   pub fn error(chain: Chain) -> Self {
      Self(Err(chain))
   }
}

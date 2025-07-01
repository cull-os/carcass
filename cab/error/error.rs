use std::{
   error,
   fmt::{
      self,
      Write as _,
   },
   io::{
      self,
      Write as _,
   },
   ops,
   process,
   result,
   sync::Arc,
};

use derive_more::Deref;
use dup::Dupe;
use ust::{
   Display,
   Write as _,
   style::StyledExt as _,
   terminal,
};

/// A type alias for concice use of [`Error`].
pub type Result<T> = result::Result<T, Error>;

/// The error type. Stores an error chain that can be appended to with
/// [`Contextful`]. Can be formatted to show the chain with [`fmt::Debug`].
#[derive(thiserror::Error, Clone, Dupe)]
#[error(transparent)]
pub struct Error(#[doc(hidden)] pub Arc<anyhow::Error>);

impl fmt::Debug for Error {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      let writer = &mut terminal::writer_from_stderr(writer);

      let mut message = String::new();

      let mut chain = self.0.chain().rev().peekable();
      while let Some(error) = chain.next() {
         message.clear();

         {
            let message = &mut terminal::writer_from_stderr(&mut message);

            terminal::indent!(
               message,
               header = if chain.peek().is_none() {
                  "error:"
               } else {
                  "cause:"
               }
               .red()
               .bold(),
            );

            match error.downcast_ref::<Displayable>() {
               Some(displayable) => displayable.display_styled(message)?,
               None => write!(message, "{error}")?,
            }

            message.finish()?;
         }

         let mut chars = message.char_indices();

         if let Some((_, first)) = chars.next()
            && let Some((second_start, second)) = chars.next()
            && second.is_lowercase()
         {
            writeln!(
               writer,
               "{first_lowercase}{rest}",
               first_lowercase = first.to_lowercase(),
               rest = &message[second_start..],
            )?;
         } else {
            writeln!(writer, "{message}")?;
         }
      }

      writer.finish()?;
      Ok(())
   }
}

#[derive(Deref)]
struct Displayable(Box<dyn Display + Send + Sync + 'static>);

impl fmt::Debug for Displayable {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      let mut writer = terminal::writer(terminal::StyleChoice::Never, writer);

      self.display_styled(&mut writer)?;

      writer.finish()?;
      Ok(())
   }
}

impl fmt::Display for Displayable {
   fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
      <Self as fmt::Debug>::fmt(self, writer)
   }
}

impl error::Error for Displayable {}

/// The termination type. Meant to be used as the return type of the main
/// function.
///
/// Can be created directly or from an `Error` with the `?` operator. Will
/// pretty print the error.
#[derive(Clone)]
pub struct Termination(Option<Error>);

impl ops::Try for Termination {
   type Output = ();
   type Residual = Termination;

   fn from_output((): Self::Output) -> Self {
      Self::success()
   }

   fn branch(self) -> ops::ControlFlow<Self::Residual, Self::Output> {
      match self.0 {
         None => ops::ControlFlow::Continue(()),
         Some(error) => ops::ControlFlow::Break(Self(Some(error))),
      }
   }
}

impl<T, E: error::Error + Send + Sync + 'static> ops::FromResidual<result::Result<T, E>>
   for Termination
{
   fn from_residual(result: result::Result<T, E>) -> Self {
      match result {
         Ok(_) => Self::success(),
         Err(error) => Self(Some(Error(Arc::new(error.into())))),
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
         None => process::ExitCode::SUCCESS,

         Some(error) => {
            let _ = write!(io::stderr(), "{error:?}");
            process::ExitCode::FAILURE
         },
      }
   }
}

impl Termination {
   /// Creates a [`Termination`] from the provided [`Error`].
   #[must_use]
   pub fn error(error: Error) -> Self {
      Self(Some(error))
   }

   /// Creates a successful [`Termination`] that returns success.
   #[must_use]
   pub fn success() -> Self {
      Self(None)
   }
}

/// Creates an [`Error`] from the provided string literal.
///
/// # Example
///
/// ```rs
/// fn get_result() -> Result<()> {
///     unimplemented!()
/// }
///
/// get_result().map_err(|error| error!("found error: {error}"))
/// ```
#[macro_export]
macro_rules! error {
   ($($t:tt)*) => {
      $crate::Error(std::sync::Arc::new($crate::private::anyhow::anyhow!($($t)*)))
   };
}

/// A macro that boils down to:
///
/// ```rs
/// return Err(error!(arguments));
/// ```
#[macro_export]
macro_rules! bail {
   ($($t:tt)*) => {{
      Err($crate::error!($($t)*))?;
      unreachable!()
   }};
}

/// The type of the context accepted by [`Contextful`].
pub trait Context = fmt::Display + Send + Sync + 'static;

/// A trait to add context to [`Error`].
pub trait Contextful<T> {
   /// Appends the context to the error chain.
   fn context(self, context: impl Context) -> Result<T>;

   fn context_tags<D: Display + Send + Sync + 'static>(self, context: impl Into<D>) -> Result<T>;

   /// Appends the context to the error chain, lazily.
   fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T>;
}

impl<T> Contextful<T> for Option<T> {
   fn context(self, context: impl Context) -> Result<T> {
      anyhow::Context::context(self, context).map_err(|error| Error(Arc::new(error)))
   }

   fn context_tags<D: Display + Send + Sync + 'static>(self, context: impl Into<D>) -> Result<T> {
      anyhow::Context::context(self, Displayable(Box::new(context.into())))
         .map_err(move |error| Error(Arc::new(error)))
   }

   fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T> {
      anyhow::Context::with_context(self, context).map_err(|error| Error(Arc::new(error)))
   }
}

impl<T, E: error::Error + Send + Sync + 'static> Contextful<T> for result::Result<T, E> {
   fn context(self, context: impl Context) -> Result<T> {
      anyhow::Context::context(self, context).map_err(|error| Error(Arc::new(error)))
   }

   fn context_tags<D: Display + Send + Sync + 'static>(self, context: impl Into<D>) -> Result<T> {
      anyhow::Context::context(self, Displayable(Box::new(context.into())))
         .map_err(move |error| Error(Arc::new(error)))
   }

   fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T> {
      anyhow::Context::with_context(self, context).map_err(|error| Error(Arc::new(error)))
   }
}

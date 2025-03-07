// ERROR

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

use yansi::Paint;

/// A type alias for consice use of [`Error`].
pub type Result<T> = result::Result<T, Error>;

/// The error type. Stores an error chain that can be appended to with
/// [`Contextful`]. Can be formatted to show the chain with [`fmt::Debug`].
#[derive(thiserror::Error, Clone)]
#[error(transparent)]
pub struct Error(#[doc(hidden)] pub Arc<anyhow::Error>);

impl fmt::Debug for Error {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut chain = self.0.chain();

        if let Some(error) = chain.next() {
            writeln!(writer, "{header} {error}", header = "error:".red().bold())?;
        }

        let mut cause = String::new();

        for error in chain {
            write!(writer, "{header} ", header = "cause:".red().bold())?;

            cause.drain(..); // TODO: Replace with .clear() when yansi removes the method that shadows.
            write!(cause, "{error}")?;

            let mut chars = cause.char_indices();

            if let Some((_, first)) = chars.next()
                && let Some((second_start, second)) = chars.next()
                && second.is_lowercase()
            {
                writeln!(
                    writer,
                    "{first_lowercase}{rest}",
                    first_lowercase = first.to_lowercase(),
                    rest = &cause[second_start..],
                )?;
            } else {
                writeln!(writer, "{cause}")?;
            }
        }

        Ok(())
    }
}

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

impl<T, E: Into<Error>> ops::FromResidual<result::Result<T, E>> for Termination {
    fn from_residual(result: result::Result<T, E>) -> Self {
        match result {
            Ok(_) => Self::success(),
            Err(error) => Self(Some(error.into())),
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
                write!(io::stderr(), "{error:?}").ok();
                process::ExitCode::FAILURE
            },
        }
    }
}

impl Termination {
    /// Creates a [`Termination`] from the provided [`Error`].
    pub fn error(error: Error) -> Self {
        Self(Some(error))
    }

    /// Creates a successful [`Termination`] that returns success.
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
        $crate::Error(::std::sync::Arc::new($crate::__private::anyhow::anyhow!($($t)*)))
    };
}

/// A macro that boils down to:
///
/// ```rs
/// return Err(error!(arguments));
/// ```
#[macro_export]
macro_rules! bail {
    ($($t:tt)*) => {
        return Err($crate::error!($($t)*));
    };
}

/// The type of the context accepted by [`Contextful`].
pub trait Context = fmt::Display + Send + Sync + 'static;

/// A trait to add context to [`Error`].
pub trait Contextful<T> {
    /// Appends the context to the error chain.
    fn context(self, context: impl Context) -> Result<T>;

    /// Appends the context to the error chain, lazily.
    fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T>;
}

impl<T> Contextful<T> for Option<T> {
    fn context(self, context: impl Context) -> Result<T> {
        anyhow::Context::context(self, context).map_err(|error| Error(Arc::new(error)))
    }

    fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T> {
        anyhow::Context::with_context(self, context).map_err(|error| Error(Arc::new(error)))
    }
}

impl<T, E: error::Error + Send + Sync + 'static> Contextful<T> for result::Result<T, E> {
    fn context(self, context: impl Context) -> Result<T> {
        anyhow::Context::context(self, context).map_err(|error| Error(Arc::new(error)))
    }

    fn with_context<C: Context>(self, context: impl FnOnce() -> C) -> Result<T> {
        anyhow::Context::with_context(self, context).map_err(|error| Error(Arc::new(error)))
    }
}

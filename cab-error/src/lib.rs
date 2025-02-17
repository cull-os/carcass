#![feature(trait_alias, try_trait_v2)]

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

use yansi::Paint as _;

#[doc(hidden)]
pub mod __private {
    pub use anyhow;
}

// ERROR

#[derive(thiserror::Error, Clone)]
#[error(transparent)]
pub struct Error(#[doc(hidden)] pub Arc<anyhow::Error>);

pub type Result<T> = result::Result<T, Error>;

impl fmt::Debug for Error {
    fn fmt(&self, writer: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = "error:".red().bold();

        for (index, error) in self.0.chain().enumerate() {
            if index == 0 {
                write!(writer, "{header} {error}")?;
            } else {
                writeln!(writer)?;

                let mut cause = String::new();
                write!(cause, "{error}").ok();

                if cause.chars().nth(1).is_some_and(|c| c.is_lowercase()) {
                    cause.replace_range(..1, &cause.chars().next().unwrap().to_lowercase().to_string());
                }

                write!(writer, "{header} caused by: {cause}")?;
            }
        }

        Ok(())
    }
}

// TERMINATION

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
            Some(e) => ops::ControlFlow::Break(Self(Some(e))),
        }
    }
}

impl<T, E: Into<Error>> ops::FromResidual<result::Result<T, E>> for Termination {
    fn from_residual(result: result::Result<T, E>) -> Self {
        match result {
            Ok(_) => Self::success(),
            Err(e) => Self(Some(e.into())),
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
    pub fn error(error: Error) -> Self {
        Self(Some(error))
    }

    pub fn success() -> Self {
        Self(None)
    }
}

// MACROS

#[macro_export]
macro_rules! error {
    ($($t:tt)*) => {
        $crate::Error(::std::sync::Arc::new($crate::__private::anyhow::anyhow!($($t)*)))
    };
}

#[macro_export]
macro_rules! bail {
    ($($t:tt)*) => {
        return Err($crate::error!($($t)*));
    };
}

// CONTEXT

pub trait Context = fmt::Display + Send + Sync + 'static;

pub trait Contextful<T> {
    fn context(self, context: impl Context) -> Result<T>;

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

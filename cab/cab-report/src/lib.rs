//! Report utilities.

#![feature(gen_blocks, if_let_guard, let_chains, trait_alias, try_trait_v2)]

#[cfg(feature = "error")] mod error;
#[cfg(feature = "error")]
pub use error::{
   Context,
   Contextful,
   Error,
   Result,
   Termination,
};

mod label;
pub use label::{
   Label,
   LabelSeverity,
};

mod point;
pub use point::Point;

mod position;
pub use position::{
   Position,
   PositionStr,
};

mod report;
pub use report::{
   Report,
   ReportLocated,
   ReportSeverity,
};

#[doc(hidden)]
pub mod private {
   pub use anyhow;
}

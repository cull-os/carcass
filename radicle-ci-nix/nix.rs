#![allow(dead_code)]

//! Wire format for Nix's `--log-format internal-json` output.

use std::{
   path,
   result,
   str::FromStr,
};

use derive_more::Display;

mod raw {
   pub mod verbosity {
      pub const ERROR: u64 = 0;
      pub const WARN: u64 = 1;
      pub const NOTICE: u64 = 2;
      pub const INFO: u64 = 3;
      pub const TALKATIVE: u64 = 4;
      pub const CHATTY: u64 = 5;
      pub const DEBUG: u64 = 6;
      pub const VOMIT: u64 = 7;
   }

   pub mod activity {
      pub const UNKNOWN: u64 = 0;
      pub const COPY_PATH: u64 = 100;
      pub const FILE_TRANSFER: u64 = 101;
      pub const REALISE: u64 = 102;
      pub const COPY_PATHS: u64 = 103;
      pub const BUILDS: u64 = 104;
      pub const BUILD: u64 = 105;
      pub const OPTIMISE_STORE: u64 = 106;
      pub const VERIFY_PATHS: u64 = 107;
      pub const SUBSTITUTE: u64 = 108;
      pub const QUERY_PATH_INFO: u64 = 109;
      pub const POST_BUILD_HOOK: u64 = 110;
      pub const BUILD_WAITING: u64 = 111;
      pub const FETCH_TREE: u64 = 112;
   }

   pub mod result {
      pub const FILE_LINKED: u64 = 100;
      pub const BUILD_LOG_LINE: u64 = 101;
      pub const UNTRUSTED_PATH: u64 = 102;
      pub const CORRUPTED_PATH: u64 = 103;
      pub const SET_PHASE: u64 = 104;
      pub const PROGRESS: u64 = 105;
      pub const SET_EXPECTED: u64 = 106;
      pub const POST_BUILD_LOG_LINE: u64 = 107;
      pub const FETCH_STATUS: u64 = 108;
   }

   #[derive(serde::Deserialize)]
   #[serde(tag = "action", rename_all = "snake_case")]
   pub(super) enum Event {
      Start {
         id:        u64,
         #[serde(rename = "parent")]
         parent_id: u64,

         level: u64,
         text:  String,

         #[serde(rename = "type")]
         kind:   u64,
         #[serde(default)]
         fields: Vec<super::Field>,
      },
      Stop {
         id: u64,
      },
      Result {
         id: u64,

         #[serde(rename = "type")]
         kind:   u64,
         #[serde(default)]
         fields: Vec<super::Field>,
      },
      #[serde(rename = "msg")]
      Message {
         level:       u64,
         #[serde(rename = "msg")]
         message:     String,
         #[serde(default, rename = "raw_msg")]
         raw_message: Option<String>,
         #[serde(default)]
         file:        Option<String>,
         #[serde(default)]
         line:        Option<u32>,
         #[serde(default)]
         column:      Option<u32>,
      },
   }
}

/// Wire prefix Nix puts before each JSON event on stderr.
pub const PREFIX: &str = "@nix ";

/// An event emitted by Nix.
#[derive(Debug, Clone)]
pub enum Event {
   /// Beginning of an activity.
   Start {
      id:        ActivityId,
      parent_id: ActivityId,

      level: Verbosity,
      text:  String,

      activity: Activity,
   },

   /// End of an activity.
   Stop { id: ActivityId },

   /// An update for an in-progress activity.
   Result { id: ActivityId, result: Result },

   /// A freeform log message not tied to any activity.
   Message {
      level:       Verbosity,
      message:     String,
      raw_message: Option<String>,
      file:        Option<path::PathBuf>,
      line:        Option<u32>,
      column:      Option<u32>,
   },
}

impl FromStr for Event {
   type Err = serde_json::Error;

   fn from_str(line: &str) -> result::Result<Self, Self::Err> {
      Ok(Self::from(serde_json::from_str::<raw::Event>(line)?))
   }
}

impl From<raw::Event> for Event {
   fn from(raw: raw::Event) -> Self {
      match raw {
         raw::Event::Start {
            id,
            parent_id,

            level,
            text,

            kind,
            fields,
         } => {
            Self::Start {
               id: ActivityId::from(id),
               parent_id: ActivityId::from(parent_id),

               level: Verbosity::from(level),
               text,

               activity: Activity::from_wire(kind, fields),
            }
         },
         raw::Event::Stop { id } => {
            Self::Stop {
               id: ActivityId::from(id),
            }
         },
         raw::Event::Result { id, kind, fields } => {
            Self::Result {
               id:     ActivityId::from(id),
               result: Result::from_wire(kind, fields),
            }
         },
         raw::Event::Message {
            level,
            message,
            raw_message,
            file,
            line,
            column,
         } => {
            Self::Message {
               level: Verbosity::from(level),
               message,
               raw_message,
               file: file.map(path::PathBuf::from),
               line,
               column,
            }
         },
      }
   }
}

/// An activity emitted by Nix.
#[derive(Debug, Clone, strum::EnumDiscriminants)]
#[strum_discriminants(name(ActivityKind), derive(Hash))]
pub enum Activity {
   /// Unknown.
   Unknown,

   /// Copying one store path between two stores.
   CopyPath {
      store_path: path::PathBuf,
      source_uri: StoreUri,
      target_uri: StoreUri,
   },

   /// Fetching one URI (HTTP download, etc.).
   FileTransfer { uri: url::Url },

   /// Realising a derivation tree (root activity).
   Realise,

   /// Copying multiple store paths (parent of `CopyPath`).
   CopyPaths,

   /// Building multiple derivations (parent of `Build`s).
   Builds,

   /// Building one derivation. `drv_path` is the `.drv` store path.
   Build {
      drv_path:      path::PathBuf,
      machine_name:  String,
      current_round: u64,
      total_rounds:  u64,
   },

   /// Optimising the local store by hard-linking duplicates.
   OptimiseStore,

   /// Verifying store paths against their NAR hashes.
   VerifyPaths,

   /// Substituting a path from a binary cache instead of building.
   Substitute {
      store_path: path::PathBuf,
      source_uri: StoreUri,
   },

   /// Querying info about a store path on a remote store.
   QueryPathInfo {
      store_path: path::PathBuf,
      source_uri: StoreUri,
   },

   /// Running the post-build hook for a derivation.
   PostBuildHook { drv_path: path::PathBuf },

   /// Waiting for a build slot, build-user id, or remote machine.
   /// Can carry the drv (and resolved drv) path.
   BuildWaiting {
      drv_path:      Option<path::PathBuf>,
      resolved_path: Option<path::PathBuf>,
   },

   /// Fetching a tree (Git repo, tarball, ...). The URI lives in
   /// the surrounding `Event::Start.text`: `"fetching Git repository <url>"`.
   FetchTree,

   /// Any activity Nix introduces that we don't model yet, or whose
   /// field shape doesn't match what we expect. The original numeric
   /// kind and field array are preserved for forward-compat.
   Other { kind: u64, fields: Vec<Field> },
}

impl From<u64> for ActivityKind {
   fn from(value: u64) -> Self {
      match value {
         raw::activity::UNKNOWN => Self::Unknown,
         raw::activity::COPY_PATH => Self::CopyPath,
         raw::activity::FILE_TRANSFER => Self::FileTransfer,
         raw::activity::REALISE => Self::Realise,
         raw::activity::COPY_PATHS => Self::CopyPaths,
         raw::activity::BUILDS => Self::Builds,
         raw::activity::BUILD => Self::Build,
         raw::activity::OPTIMISE_STORE => Self::OptimiseStore,
         raw::activity::VERIFY_PATHS => Self::VerifyPaths,
         raw::activity::SUBSTITUTE => Self::Substitute,
         raw::activity::QUERY_PATH_INFO => Self::QueryPathInfo,
         raw::activity::POST_BUILD_HOOK => Self::PostBuildHook,
         raw::activity::BUILD_WAITING => Self::BuildWaiting,
         raw::activity::FETCH_TREE => Self::FetchTree,
         _ => Self::Other,
      }
   }
}

macro_rules! match_fields {
   ($kind:ident, $fields:ident, $fields_pattern:pat => try $body:block) => {
      match &*$fields {
         $fields_pattern if let Some(result) = try $body => result,
         _ => Self::Other { kind: $kind, fields: $fields },
      }
   };

   ($kind:ident, $fields:ident, $fields_pattern:pat => $body:expr) => {
      match &*$fields {
         $fields_pattern => $body,
         _ => Self::Other { kind: $kind, fields: $fields },
      }
   };
}

impl Activity {
   #[expect(clippy::cognitive_complexity)]
   fn from_wire(kind: u64, fields: Vec<Field>) -> Self {
      match kind {
         raw::activity::UNKNOWN if fields.is_empty() => Self::Unknown,
         raw::activity::COPY_PATH => {
            match_fields!(kind, fields, &[
               Field::String(ref store_path),
               Field::String(ref source_uri),
               Field::String(ref target_uri),
            ] => try {
               Self::CopyPath {
                  store_path: path::PathBuf::from(store_path),
                  source_uri: StoreUri::from_str(source_uri).ok()?,
                  target_uri: StoreUri::from_str(target_uri).ok()?,
                }
             })
         },
         raw::activity::FILE_TRANSFER => {
            match_fields!(kind, fields, &[Field::String(ref uri)] => try {
               Self::FileTransfer {
                  uri: url::Url::from_str(uri).ok()?,
               }
            })
         },
         raw::activity::REALISE if fields.is_empty() => Self::Realise,
         raw::activity::COPY_PATHS if fields.is_empty() => Self::CopyPaths,
         raw::activity::BUILDS if fields.is_empty() => Self::Builds,
         raw::activity::BUILD => {
            match_fields!(kind, fields, &[
               Field::String(ref drv_path),
               Field::String(ref machine_name),
               Field::Number(ref current_round),
               Field::Number(ref total_rounds),
            ] => try {
               Self::Build {
                  drv_path: path::PathBuf::from(drv_path),
                  machine_name: machine_name.clone(),
                  current_round: current_round.as_u64()?,
                  total_rounds:  total_rounds.as_u64()?,
               }
            })
         },
         raw::activity::OPTIMISE_STORE if fields.is_empty() => Self::OptimiseStore,
         raw::activity::VERIFY_PATHS if fields.is_empty() => Self::VerifyPaths,
         raw::activity::SUBSTITUTE => {
            match_fields!(kind, fields, &[Field::String(ref store_path), Field::String(ref source_uri)] => try {
              Self::Substitute {
                 store_path: path::PathBuf::from(store_path),
                 source_uri: StoreUri::from_str(source_uri).ok()?,
               }
            })
         },
         raw::activity::QUERY_PATH_INFO => {
            match_fields!(kind, fields, &[Field::String(ref store_path), Field::String(ref source_uri)] => try {
              Self::QueryPathInfo {
                 store_path: path::PathBuf::from(store_path),
                 source_uri: StoreUri::from_str(source_uri).ok()?,
               }
            })
         },
         raw::activity::POST_BUILD_HOOK => {
            match_fields!(kind, fields, &[Field::String(ref drv_path)] => {
               Self::PostBuildHook {
                  drv_path: path::PathBuf::from(drv_path),
                }
            })
         },
         raw::activity::BUILD_WAITING => {
            match &*fields {
               &[] => {
                  Self::BuildWaiting {
                     drv_path:      None,
                     resolved_path: None,
                  }
               },
               &[Field::String(ref drv_path)] => {
                  Self::BuildWaiting {
                     drv_path:      Some(path::PathBuf::from(drv_path)),
                     resolved_path: None,
                  }
               },
               &[
                  Field::String(ref drv_path),
                  Field::String(ref resolved_path),
               ] => {
                  Self::BuildWaiting {
                     drv_path:      Some(path::PathBuf::from(drv_path)),
                     resolved_path: Some(path::PathBuf::from(resolved_path)),
                  }
               },
               _ => Self::Other { kind, fields },
            }
         },
         raw::activity::FETCH_TREE if fields.is_empty() => Self::FetchTree,
         _ => Self::Other { kind, fields },
      }
   }
}

#[derive(Debug, Clone)]
pub enum Result {
   /// A duplicate file hard-linked during store optimisation.
   /// `blocks` is `None` on Windows where `st_blocks` doesn't exist. (LOL - Nix
   /// on Windows!)
   FileLinked { size: u64, blocks: Option<u64> },

   /// One line of stdout/stderr from the builder.
   BuildLogLine { line: String },

   /// A path signed by an untrusted key.
   UntrustedPath { store_path: path::PathBuf },

   /// A path that failed signature verification.
   CorruptedPath { store_path: path::PathBuf },

   /// The builder entering a new lifecycle phase.
   SetPhase { phase: Phase },

   /// Progress counters for any activity. The unit of work is
   /// activity-defined, so these are not always item counts.
   Progress {
      /// Units that reached a terminal, non-failed state. This is not
      /// always "success" in a human sense:
      ///
      /// For `actVerifyPaths`, a path can count as done while still producing
      /// `UntrustedPath` or `CorruptedPath`.
      ///
      /// For file transfers, `done` is bytes transferred.
      ///
      /// And so on.
      done:   u64,
      /// Units that failed.
      failed: u64,

      /// Units currently in progress.
      running: u64,

      /// Total expected units of work.
      expected: u64,
   },

   /// The expected count for `activity_kind` (an
   /// [`ActivityKind`], not the parent activity's [`ActivityId`]).
   SetExpected {
      activity_kind: ActivityKind,
      count:         u64,
   },

   /// One line from the post-build hook's stdout/stderr.
   PostBuildLogLine { line: String },

   /// A status update during a fetch.
   FetchStatus { status: String },

   /// Any result type Nix introduces that we don't model yet, or whose
   /// field shape doesn't match what we expect.
   Other { kind: u64, fields: Vec<Field> },
}

impl Result {
   fn from_wire(kind: u64, fields: Vec<Field>) -> Self {
      match kind {
         raw::result::FILE_LINKED => {
            match &*fields {
               &[Field::Number(ref size)] if let Some(size) = try { size.as_u64()? } => {
                  Self::FileLinked { size, blocks: None }
               },
               &[Field::Number(ref size), Field::Number(ref blocks)]
                  if let Some((size, blocks)) = try { (size.as_u64()?, blocks.as_u64()?) } =>
               {
                  Self::FileLinked {
                     size,
                     blocks: Some(blocks),
                  }
               },
               _ => Self::Other { kind, fields },
            }
         },
         raw::result::BUILD_LOG_LINE => {
            match_fields!(kind, fields, &[Field::String(ref line)] => Self::BuildLogLine { line: line.clone() })
         },
         raw::result::UNTRUSTED_PATH => {
            match_fields!(kind, fields, &[Field::String(ref store_path)] => {
               Self::UntrustedPath {
                  store_path: path::PathBuf::from(store_path),
               }
            })
         },
         raw::result::CORRUPTED_PATH => {
            match_fields!(kind, fields, &[Field::String(ref store_path)] => {
               Self::CorruptedPath {
                  store_path: path::PathBuf::from(store_path),
               }
            })
         },
         raw::result::SET_PHASE => {
            match_fields!(kind, fields, &[Field::String(ref phase)] => {
               Self::SetPhase {
                  phase: Phase::from(phase.clone()),
               }
            })
         },
         raw::result::PROGRESS => {
            match_fields!(kind, fields, &[
               Field::Number(ref done),
               Field::Number(ref expected),
               Field::Number(ref running),
               Field::Number(ref failed),
            ] => try {
               Self::Progress {
                  done:     done.as_u64()?,
                  expected: expected.as_u64()?,
                  running:  running.as_u64()?,
                  failed:   failed.as_u64()?,
               }
            })
         },
         raw::result::SET_EXPECTED => {
            match_fields!(kind, fields, &[Field::Number(ref activity_kind), Field::Number(ref count)] => try {
               Self::SetExpected {
                  activity_kind: ActivityKind::from(activity_kind.as_u64()?),
                  count:         count.as_u64()?,
               }
            })
         },
         raw::result::POST_BUILD_LOG_LINE => {
            match_fields!(kind, fields, &[Field::String(ref line)] => Self::PostBuildLogLine { line: line.clone() })
         },
         raw::result::FETCH_STATUS => {
            match_fields!(kind, fields, &[Field::String(ref status)] => Self::FetchStatus { status: status.clone() })
         },
         _ => Self::Other { kind, fields },
      }
   }
}

/// Identifier for an in-progress activity.
#[derive(
   Debug, Display, Clone, Copy, Eq, PartialEq, Hash, derive_more::From, derive_more::Into,
)]
pub struct ActivityId(u64);

/// Verbosity at which an activity or message was emitted.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Verbosity {
   Error,
   Warn,
   Notice,
   Info,
   Talkative,
   Chatty,
   Debug,
   Vomit,
   Unknown(u64),
}

impl From<u64> for Verbosity {
   fn from(value: u64) -> Self {
      match value {
         raw::verbosity::ERROR => Self::Error,
         raw::verbosity::WARN => Self::Warn,
         raw::verbosity::NOTICE => Self::Notice,
         raw::verbosity::INFO => Self::Info,
         raw::verbosity::TALKATIVE => Self::Talkative,
         raw::verbosity::CHATTY => Self::Chatty,
         raw::verbosity::DEBUG => Self::Debug,
         raw::verbosity::VOMIT => Self::Vomit,
         other => Self::Unknown(other),
      }
   }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum StoreUri {
   Auto,
   Daemon,
   Local,
   Url(url::Url),
}

impl FromStr for StoreUri {
   type Err = url::ParseError;

   fn from_str(value: &str) -> result::Result<Self, Self::Err> {
      Ok(match value {
         "auto" => Self::Auto,
         "daemon" => Self::Daemon,
         "local" => Self::Local,
         other => Self::Url(url::Url::parse(other)?),
      })
   }
}

#[derive(Debug, Clone, Eq, PartialEq, strum::EnumString)]
pub enum Phase {
   #[strum(serialize = "unpackPhase")]
   Unpack,
   #[strum(serialize = "patchPhase")]
   Patch,
   #[strum(serialize = "configurePhase")]
   Configure,
   #[strum(serialize = "buildPhase")]
   Build,
   #[strum(serialize = "checkPhase")]
   Check,
   #[strum(serialize = "installPhase")]
   Install,
   #[strum(serialize = "fixupPhase")]
   Fixup,
   #[strum(serialize = "installCheckPhase")]
   InstallCheck,
   #[strum(serialize = "distPhase")]
   Dist,
   #[strum(disabled)]
   Other(String),
}

impl From<String> for Phase {
   fn from(value: String) -> Self {
      Self::from_str(&value).unwrap_or(Self::Other(value))
   }
}

pub use serde_json::Value as Field;

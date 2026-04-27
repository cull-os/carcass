//! Wire-format message types shared with the Radicle CI broker.
//!
//! These mirror `radicle_ci_broker::msg` so the broker can spawn this
//! adapter without protocol changes. Only the fields the adapter
//! actually reads are declared here; unknown JSON fields are ignored.
//! Long-term this module should move to its own crate that the broker
//! and adapters both depend on.

use std::io;

use derive_more::Display;
use radicle::{
   git as radicle_git,
   identity as radicle_identity,
};

/// Declare a serde `with`-module that (de)serializes a value at the end of a
/// nested JSON path. The module is named after the keys joined with `_`.
/// JSON-only; leaf type must be `DeserializeOwned`.
///
/// ```ignore
/// serde_path!["repository", "id"]; // defines `mod repository_id { ... }`
///
/// #[derive(serde::Deserialize)]
/// struct Request {
///    #[serde(rename = "common", with = "repository_id")]
///    repo_id: RepoId,
/// }
/// ```
macro_rules! serde_path {
   [$($vis:vis ,)? $first:literal $(, $rest:literal)* $(,)?] => {
      paste::paste! {
         #[expect(clippy::allow_attributes)]
         #[allow(dead_code, reason = "macro generates both directions; not all callers need each")]
         $($vis)? mod [<$first $(_ $rest)*>] {
            use serde::{
               Deserialize as _,
               Serialize as _,
               de::{
                  self as serde_de,
                  Error as _,
               },
               ser::{
                  self as serde_ser,
                  Error as _,
               },
            };

            pub fn deserialize<'de, T: serde_de::DeserializeOwned, D: serde_de::Deserializer<'de>>(
               deserializer: D,
            ) -> Result<T, D::Error> {
               let mut value = serde_json::Value::deserialize(deserializer)?;

               for key in [$first $(, $rest)*] {
                  value = match value {
                     serde_json::Value::Object(mut map) => {
                        map.remove(key)
                           .ok_or_else(|| D::Error::custom(format_args!("missing key '{key}'")))?
                     },
                     _ => {
                        return Err(D::Error::custom(format_args!(
                           "expected an object containing key '{key}'",
                        )));
                     },
                  };
               }

               serde_json::from_value(value).map_err(D::Error::custom)
            }

            #[allow(dead_code)]
            pub fn serialize<T: serde::Serialize, S: serde_ser::Serializer>(
               value: &T,
               serializer: S,
            ) -> Result<S::Ok, S::Error> {
               let mut leaf = serde_json::to_value(value).map_err(S::Error::custom)?;

               for key in [$first $(, $rest)*].into_iter().rev() {
                  leaf = serde_json::Value::Object(serde_json::Map::from_iter([(
                     key.to_owned(),
                     leaf,
                  )]));
               }

               leaf.serialize(serializer)
            }
         }
      }
   };
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to deserialize request")]
   DeserializeRequest(#[source] serde_json::Error),

   #[error("failed to serialize response")]
   SerializeResponse(#[source] serde_json::Error),

   #[error("failed to write response")]
   WriteResponse(#[source] io::Error),

   #[error("request did not specify any commits")]
   NoCommits,
}

#[derive(Debug, Display, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RunId {
   id: uuid::Uuid,
}

impl RunId {
   pub fn generate() -> Self {
      Self {
         id: uuid::Uuid::new_v4(),
      }
   }
}

#[derive(Debug, Clone, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunResult {
   Success,
   Failure,
}

mod path {
   serde_path![pub, "id"];
   serde_path![pub, "commits"];
}

/// A request message sent by the broker to its adapter child process.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Request {
   Trigger {
      #[serde(rename = "repository", with = "path::id")]
      repo_id: radicle_identity::RepoId,

      #[serde(flatten)]
      event: TriggerEvent,
   },
}

/// The event that produced a [`Request::Trigger`]. Exactly one of these
/// is present per trigger; serde picks the variant by which fields the
/// JSON object carries.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub enum TriggerEvent {
   Push {
      commits: Vec<radicle_git::Oid>,
   },
   Patch {
      #[serde(rename = "patch", with = "path::commits")]
      commits: Vec<radicle_git::Oid>,
   },
}

impl Request {
   pub fn from_reader<R: io::Read>(reader: R) -> Result<Self, Error> {
      serde_json::from_reader(reader).map_err(Error::DeserializeRequest)
   }

   pub fn repo_id(&self) -> radicle_identity::RepoId {
      match *self {
         Self::Trigger { repo_id, .. } => repo_id,
      }
   }

   /// The tip commit the broker is asking CI to run against.
   ///
   /// For a push this is the new branch head, for a patch this is the latest
   /// revision's tip.
   pub fn tip_oid(&self) -> Result<radicle_git::Oid, Error> {
      match *self {
         Self::Trigger {
            event: TriggerEvent::Push { ref commits } | TriggerEvent::Patch { ref commits },
            ..
         } => commits,
      }
      .first()
      .copied()
      .ok_or(Error::NoCommits)
   }
}

/// A response message from the adapter child process to the broker.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "response", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Response {
   Triggered {
      run_id:   RunId,
      info_url: Option<url::Url>,
   },
   Finished {
      result: RunResult,
   },
}

impl Response {
   pub fn to_writer<W: io::Write>(&self, mut writer: W) -> Result<(), Error> {
      let mut line = serde_json::to_string(self).map_err(Error::SerializeResponse)?;
      line.push('\n');
      writer
         .write_all(line.as_bytes())
         .map_err(Error::WriteResponse)
   }
}

#[cfg(test)]
mod tests {
   use super::*;

   const REPO_ID: &str = "rad:z2AeopTVY58JJSJC3zroqEV5T3pNp";
   const COMMIT_A: &str = "1d6c2af010531a0937a8b4eb70afeaf45f6db19c";
   const COMMIT_B: &str = "4af58bafe75d2ea4ac5cfe4b1797fd5ae00b87bb";

   const EXPECT_DESERIALIZE: &str = "static json fixture must deserialize";
   const EXPECT_SERIALIZE: &str = "static value must serialize";

   #[test]
   fn push_trigger_yields_tip_oid() {
      let request = serde_json::from_value::<Request>(serde_json::json!({
         "request": "trigger",
         "version": 1_u32,
         "event_type": "push",
         "repository": { "id": REPO_ID },
         "branch": "main",
         "commits": [COMMIT_A, COMMIT_B],
      }))
      .expect(EXPECT_DESERIALIZE);

      assert_eq!(request.repo_id().to_string(), REPO_ID);
      assert_eq!(
         request
            .tip_oid()
            .expect("trigger must carry a tip commit")
            .to_string(),
         COMMIT_A
      );
   }

   #[test]
   fn patch_trigger_yields_tip_oid() {
      let request = serde_json::from_value::<Request>(serde_json::json!({
         "request": "trigger",
         "version": 1_u32,
         "event_type": "patch",
         "repository": { "id": REPO_ID },
         "action": "created",
         "patch": {
            "id": "deadbeef",
            "title": "test patch",
            "commits": [COMMIT_B],
         },
      }))
      .expect(EXPECT_DESERIALIZE);

      assert_eq!(request.repo_id().to_string(), REPO_ID);
      assert_eq!(
         request
            .tip_oid()
            .expect("trigger must carry a tip commit")
            .to_string(),
         COMMIT_B
      );
   }

   #[test]
   fn missing_repository_id_fails_to_deserialize() {
      let error = serde_json::from_value::<Request>(serde_json::json!({
         "request": "trigger",
         "repository": {},
         "commits": [COMMIT_A],
      }))
      .expect_err("missing nested 'id' key must surface an error");
      assert!(error.to_string().contains("'id'"), "got: {error}");
   }

   #[test]
   fn empty_commits_array_yields_no_commits_error() {
      let request = serde_json::from_value::<Request>(serde_json::json!({
         "request": "trigger",
         "repository": { "id": REPO_ID },
         "commits": [],
      }))
      .expect(EXPECT_DESERIALIZE);
      assert!(matches!(request.tip_oid(), Err(Error::NoCommits)));
   }

   #[test]
   fn response_variants_serialize_to_broker_wire_format() {
      let run_id = RunId::generate();
      assert_eq!(
         serde_json::to_value(Response::Triggered {
            run_id:   run_id.clone(),
            info_url: Some(
               url::Url::parse("https://ci.example/run").expect("literal must be valid")
            ),
         })
         .expect(EXPECT_SERIALIZE),
         serde_json::json!({
            "response": "triggered",
            "run_id": run_id,
            "info_url": "https://ci.example/run",
         }),
      );

      assert_eq!(
         serde_json::to_value(Response::Finished {
            result: RunResult::Failure,
         })
         .expect(EXPECT_SERIALIZE),
         serde_json::json!({ "response": "finished", "result": "failure" }),
      );
   }

   #[test]
   fn serde_path_macro_round_trips_through_nested_object() {
      #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
      struct Outer {
         #[serde(rename = "repository", with = "path::id")]
         id: radicle_identity::RepoId,
      }

      let original = Outer {
         id: REPO_ID
            .parse()
            .expect("REPO_ID was constructed from a valid radicle id"),
      };

      let serialized = serde_json::to_value(&original).expect(EXPECT_SERIALIZE);
      assert_eq!(
         serialized,
         serde_json::json!({ "repository": { "id": REPO_ID } }),
      );

      let deserialized = serde_json::from_value(serialized).expect(EXPECT_DESERIALIZE);
      assert_eq!(original, deserialized);
   }
}

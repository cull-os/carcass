#![allow(dead_code)]

//! Configuration related utilities.

use std::{
   fs,
   io,
   path,
};

use radicle::profile as radicle_profile;

#[derive(Debug, thiserror::Error)]
pub enum Error {
   #[error("failed to load radicle profile")]
   LoadProfile(#[source] radicle_profile::Error),

   #[error("failed to read config file '{path}'", path = path.display())]
   Read {
      path:   path::PathBuf,
      #[source]
      source: io::Error,
   },

   #[error("failed to parse config file '{path}'", path = path.display())]
   Parse {
      path:   path::PathBuf,
      #[source]
      source: serde_json::Error,
   },

   #[error("base URL '{0}' must be hierarchical (e.g., http://..., not mailto:...)")]
   InvalidBaseUrl(url::Url),

   #[error("base URL '{0}' must not end with a trailing slash")]
   TrailingBaseUrlSlash(url::Url),
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ConfigFile {
   base_url: Option<url::Url>,
}

#[derive(Debug)]
pub struct Config {
   profile:  radicle_profile::Profile,
   base_url: Option<url::Url>,
}

impl Config {
   pub fn load() -> Result<Self, Error> {
      let profile = radicle_profile::Profile::load().map_err(Error::LoadProfile)?;
      let path = profile.home.path().join("ci-nix").join("config.json");

      let file = match fs::read_to_string(&path) {
         Ok(text) => {
            serde_json::from_str::<ConfigFile>(&text).map_err(|source| {
               Error::Parse {
                  path: path.clone(),
                  source,
               }
            })?
         },
         Err(error) if error.kind() == io::ErrorKind::NotFound => ConfigFile::default(),
         Err(source) => return Err(Error::Read { path, source }),
      };

      let base_url = match file.base_url {
         Some(url) if url.cannot_be_a_base() => return Err(Error::InvalidBaseUrl(url)),
         // `Url` normalizes a missing path (https://foo.example) to "/", so the bare-host case must be excluded.
         Some(url) if url.path() != "/" && url.path().ends_with('/') => {
            return Err(Error::TrailingBaseUrlSlash(url));
         },
         other => other,
      };

      Ok(Self { profile, base_url })
   }

   pub fn profile(&self) -> &radicle_profile::Profile {
      &self.profile
   }

   pub fn run_path(
      &self,
      segments: impl IntoIterator<Item = impl AsRef<path::Path>>,
   ) -> path::PathBuf {
      let mut path = self.profile.home.path().join("ci-nix").join("runs");
      path.extend(segments);
      path
   }

   pub fn run_url(&self, segments: impl IntoIterator<Item = impl AsRef<str>>) -> Option<url::Url> {
      let mut url = self.base_url.clone()?;
      url.path_segments_mut()
         .expect("base url must be hierarchical")
         .extend(segments);
      Some(url)
   }
}

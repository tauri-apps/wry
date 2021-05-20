// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Re-exported Tao APIs
//!
//! This module re-export [tao] APIs for user to create application windows. To learn more about
//! how to use tao, please see [its documentation](https://crates.io/crates/tao).
//!
//! [tao]: https://crates.io/crates/tao

pub use tao::*;

/// A single browser context.
///
/// Think of this like a browser session. Incognito mode would be a single context even though
/// it has multiple tab/windows.
pub struct Application {
  inner: ApplicationInner,
}

impl Application {
  pub fn new(data_directory: Option<std::path::PathBuf>) -> Self {
    Self {
      inner: ApplicationInner::new(data_directory),
    }
  }
}

#[cfg(target_os = "linux")]
use self::unix::ApplicationInner;

#[cfg(target_os = "windows")]
use self::windows::ApplicationInner;

#[cfg(target_os = "macos")]
use self::macos::ApplicationInner;

#[cfg(target_os = "linux")]
#[cfg_attr(doc_cfg, doc(cfg(target_os = "linux")))]
pub mod unix {
  //! Unix platform extensions for [`Application`](super::Application).
  use std::{path::PathBuf};
  use webkit2gtk::{WebContext, WebContextBuilder,  WebsiteDataManagerBuilder};

  pub(crate) struct ApplicationInner {
    context: WebContext,
    automation: bool,
  }

  impl ApplicationInner {
    pub fn new(data_directory: Option<PathBuf>) -> Self {
      let mut context_builder = WebContextBuilder::new();
      if let Some(data_directory) = data_directory {
        let data_manager = WebsiteDataManagerBuilder::new()
          .local_storage_directory(
            &data_directory
              .join("localstorage")
              .to_string_lossy()
              .into_owned(),
          )
          .indexeddb_directory(
            &data_directory
              .join("databases")
              .join("indexeddb")
              .to_string_lossy()
              .into_owned(),
          )
          .build();
        context_builder = context_builder.website_data_manager(&data_manager);
      }

      Self {
        context: context_builder.build(),
        automation: true,
      }
    }
  }

  /// [`Application`](super::Application) items that only matter on unix.
  pub trait ApplicationExt {
    /// The context of all webviews opened.
    fn context(&self) -> &WebContext;

    /// If the context allows automation.
    ///
    /// **Note:** `libwebkit2gtk` only allows 1 automation context at a time.
    fn allows_automation(&self) -> bool;

    /// Set if this context allows automation (default: `true`).
    fn set_automation_mode(&mut self, flag: bool);
  }

  impl ApplicationExt for super::Application {
    fn context(&self) -> &WebContext {
      &self.inner.context
    }

    fn allows_automation(&self) -> bool {
      self.inner.automation
    }

    fn set_automation_mode(&mut self, flag: bool) {
      self.inner.automation = flag;
    }
  }
}

#[cfg(target_os = "windows")]
#[cfg_attr(doc_cfg, doc(cfg(target_os = "windows")))]
pub(crate) mod windows {
  use std::{
    env::var,
    path::{Path, PathBuf},
  };

  pub struct ApplicationInner {
    data_directory: Option<PathBuf>,
  }

  impl ApplicationInner {
    pub fn new(data_directory: Option<PathBuf>) -> Self {
      Self {
        data_directory,
        automation,
      }
    }
  }

  /// [`Application`](super::Application) items that only matter on windows.
  pub trait ApplicationExt {
    fn data_directory(&self) -> Option<&Path>;
  }

  impl ApplicationExt for super::Application {
    fn data_directory(&self) -> Option<&Path> {
      self.inner.data_directory.as_deref()
    }
  }
}

#[cfg(target_os = "macos")]
#[cfg_attr(doc_cfg, doc(cfg(target_os = "macos")))]
pub(crate) mod macos {
  use std::{env::var, path::PathBuf};

  pub struct ApplicationInner {
    automation: bool,
  }

  impl ApplicationInner {
    pub fn new(_data_directory: Option<PathBuf>) -> Self {
      let automation = var("TAURI_AUTOMATION_MODE").as_deref() == Ok("TRUE");
      Self { automation }
    }
  }
}

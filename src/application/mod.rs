// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Re-exported Tao APIs
//!
//! This module re-export [tao] APIs for user to create application windows. To learn more about
//! how to use tao, please see [its documentation](https://crates.io/crates/tao).
//!
//! [tao]: https://crates.io/crates/tao

use std::path::PathBuf;
pub use tao::*;

/// A single browser context.
///
/// Think of this like a browser session. Incognito mode would be a single context even though
/// it has multiple tab/windows.
pub struct Application {
  inner: ApplicationInner,
}

impl Application {
  pub fn new(data_directory: Option<PathBuf>) -> Self {
    Self {
      inner: ApplicationInner::new(data_directory),
    }
  }
}

#[cfg(target_os = "linux")]
use self::gtk::ApplicationInner;

#[cfg(target_os = "linux")]
pub(crate) mod gtk {
  use std::{env::var, path::PathBuf};
  use webkit2gtk::{
    AutomationSessionBuilder, AutomationSessionExt, UserContentManager, WebContext,
    WebContextBuilder, WebContextExt, WebViewExtManual, WebsiteDataManagerBuilder,
  };

  pub struct ApplicationInner {
    context: WebContext,
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

      let context = context_builder.build();

      let automation = var("TAURI_AUTOMATION_MODE").as_deref() == Ok("1");
      dbg!(automation);
      context.set_automation_allowed(automation);

      Self { context }
    }
  }

  pub trait ApplicationGtkExt {
    fn context(&self) -> &WebContext;
  }

  impl ApplicationGtkExt for super::Application {
    fn context(&self) -> &WebContext {
      &self.inner.context
    }
  }
}

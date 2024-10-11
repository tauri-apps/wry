// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(gtk)]
use crate::webkitgtk::WebContextImpl;

use std::{
  collections::HashSet,
  path::{Path, PathBuf},
};

/// A context that is shared between multiple [`WebView`]s.
///
/// A browser would have a context for all the normal tabs and a different context for all the
/// private/incognito tabs.
///
/// # Warning
/// If [`WebView`] is created by a WebContext. Dropping `WebContext` will cause [`WebView`] lose
/// some actions like custom protocol on Mac. Please keep both instances when you still wish to
/// interact with them.
///
/// [`WebView`]: crate::WebView
#[derive(Debug)]
pub struct WebContext {
  data_directory: Option<PathBuf>,
  #[allow(dead_code)] // It's not needed on Windows and macOS.
  pub(crate) os: WebContextImpl,
  #[allow(dead_code)] // It's not needed on Windows and macOS.
  pub(crate) custom_protocols: HashSet<String>,
}

impl WebContext {
  /// Create a new [`WebContext`].
  ///
  /// `data_directory`:
  /// * Whether the WebView window should have a custom user data path. This is useful in Windows
  ///   when a bundled application can't have the webview data inside `Program Files`.
  pub fn new(data_directory: Option<PathBuf>) -> Self {
    Self {
      os: WebContextImpl::new(data_directory.as_deref()),
      data_directory,
      custom_protocols: Default::default(),
    }
  }

  #[cfg(gtk)]
  pub(crate) fn new_ephemeral() -> Self {
    Self {
      os: WebContextImpl::new_ephemeral(),
      data_directory: None,
      custom_protocols: Default::default(),
    }
  }

  /// A reference to the data directory the context was created with.
  pub fn data_directory(&self) -> Option<&Path> {
    self.data_directory.as_deref()
  }

  #[allow(dead_code)]
  pub(crate) fn register_custom_protocol(&mut self, name: String) -> Result<(), crate::Error> {
    if self.custom_protocols.contains(&name) {
      return Err(crate::Error::ContextDuplicateCustomProtocol(name));
    }

    Ok(())
  }

  /// Check if a custom protocol has been registered on this context.
  pub fn is_custom_protocol_registered(&self, name: String) -> bool {
    self.custom_protocols.contains(&name)
  }

  /// Set if this context allows automation.
  ///
  /// **Note:** This is currently only enforced on Linux, and has the stipulation that
  /// only 1 context allows automation at a time.
  pub fn set_allows_automation(&mut self, flag: bool) {
    self.os.set_allows_automation(flag);
  }
}

impl Default for WebContext {
  fn default() -> Self {
    Self::new(None)
  }
}

#[cfg(not(gtk))]
#[derive(Debug)]
pub(crate) struct WebContextImpl;

#[cfg(not(gtk))]
impl WebContextImpl {
  fn new(_: Option<&Path>) -> Self {
    Self
  }

  fn set_allows_automation(&mut self, _flag: bool) {}
}

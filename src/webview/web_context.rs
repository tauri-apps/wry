#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
use crate::webview::webkitgtk::WebContextImpl;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::webview::wkwebview::WebContextImpl;

use std::{path::{Path, PathBuf}, sync::Arc};

/// A context that is shared between multiple [`WebView`]s.
///
/// A browser would have a context for all the normal tabs and a different context for all the
/// private/incognito tabs.
///
/// # Warning
/// If [`Webview`] is created by a WebContext. Dropping `WebContext` will cause [`WebView`] lose
/// some actions like custom protocol on Mac. Please keep both instances when you still wish to
/// interact with them.
///
/// [`WebView`]: crate::webview::WebView
#[derive(Debug)]
pub struct WebContext {
  data: WebContextData,
  #[allow(dead_code)] // It's not needed on Windows and macOS.
  pub(crate) os: WebContextImpl,
}

impl WebContext {
  /// Create a new [`WebContext`].
  ///
  /// `data_directory`:
  /// * Whether the WebView window should have a custom user data path. This is useful in Windows
  ///   when a bundled application can't have the webview data inside `Program Files`.
  pub fn new(data_directory: Option<PathBuf>) -> Self {
    let data = WebContextData { data_directory, nav_callback: None };
    let os = WebContextImpl::new(&data);
    Self { data, os }
  }

  /// A reference to the data directory the context was created with.
  pub fn data_directory(&self) -> Option<&Path> {
    self.data.data_directory()
  }

  /// Set if this context allows automation.
  ///
  /// **Note:** This is currently only enforced on Linux, and has the stipulation that
  /// only 1 context allows automation at a time.
  pub fn set_allows_automation(&mut self, flag: bool) {
    self.os.set_allows_automation(flag);
  }

  pub fn set_navigation_callback(&mut self, callback: impl NavCallback) {
    self.data.nav_callback = Some(Arc::new(callback))
  }

  pub fn navigation_callback(&self) -> Option<&Arc<dyn NavCallback>> {
    self.data.nav_callback()
  }
}

impl Default for WebContext {
  fn default() -> Self {
    let data = WebContextData::default();
    let os = WebContextImpl::new(&data);
    Self { data, os }
  }
}

pub trait NavCallback: Fn(String, bool) -> bool + 'static {}

impl<T: Fn(String, bool) -> bool + 'static> NavCallback for T {}

/// Data that all [`WebContext`] share regardless of platform.
#[derive(Default)]
pub struct WebContextData {
  data_directory: Option<PathBuf>,
  nav_callback: Option<Arc<dyn NavCallback>>
}

impl WebContextData {
  /// A reference to the data directory the context was created with.
  pub fn data_directory(&self) -> Option<&Path> {
    self.data_directory.as_deref()
  }

  pub fn nav_callback(&self) -> Option<&Arc<dyn NavCallback>> {
    self.nav_callback.as_ref()
  }
}

impl std::fmt::Debug for WebContextData {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("WebContextData").field("data_directory", &self.data_directory).field("nav_callback", &self.nav_callback.is_some()).finish()
  }
}

#[cfg(target_os = "windows")]
#[derive(Debug)]
pub(crate) struct WebContextImpl;

#[cfg(target_os = "windows")]
impl WebContextImpl {
  fn new(_data: &WebContextData) -> Self {
    Self
  }

  fn set_allows_automation(&mut self, _flag: bool) {}
}

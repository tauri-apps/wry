// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(gtk)]
use crate::webkitgtk::WebContextImpl;

#[cfg(gtk)]
use std::time::Duration;
use std::{
  collections::HashSet,
  path::{Path, PathBuf},
};

/// A context that is shared between multiple [`WebView`]s.
///
/// A browser would have a context for all the normal tabs and a different context for all the
/// private/incognito tabs.
///
/// ## Platform-specific
///
/// - **Linux**: [`WebContextExtUnix::new_with_memory_pressure_settings`] creates a context whose
///   WebKitGTK web processes use custom memory-pressure settings.
///
/// # Warning
///
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
  /// - `data_directory`: Whether the WebView window should have a custom user data path.
  ///   This is useful in Windows when a bundled application can't have the webview data inside `Program Files`.
  ///
  /// ## Platform-specific:
  ///
  /// - **Windows**: Webview instances with different `CoreWebView2EnvironmentOptions` must have different `data_directory`s [^1]
  ///
  /// [^1]: <https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2environment.createcorewebview2controllerasync?view=webview2-dotnet-1.0.3719.77#:~:text=WebView%20creation%20fails%20if%20a%20running%20instance%20using%20the%20same%20user%20data%20folder%20exists%2C%20and%20the%20Environment%20objects%20have%20different%20CoreWebView2EnvironmentOptions.>
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

  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  pub(crate) fn register_custom_protocol(&mut self, name: String) -> Result<(), crate::Error> {
    if self.is_custom_protocol_registered(&name) {
      return Err(crate::Error::ContextDuplicateCustomProtocol(name));
    }
    self.custom_protocols.insert(name);
    Ok(())
  }

  /// Check if a custom protocol has been registered on this context.
  pub fn is_custom_protocol_registered(&self, name: &str) -> bool {
    self.custom_protocols.contains(name)
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

/// WebKitGTK memory-pressure settings for the web processes of a [`WebContext`].
///
/// WebKitGTK measures the memory of every web process once per `poll_interval` and, when it
/// exceeds a fraction of `memory_limit_mb`, releases non-critical memory (the conservative
/// policy), also releases critical memory (the strict policy) or kills the process. Every field
/// left as `None` keeps WebKitGTK's default, listed below. WebKitGTK does not accept values
/// outside the ranges below: it logs a critical warning and keeps the previous value.
///
/// See WebKitGTK's [`WebKitMemoryPressureSettings`](https://webkitgtk.org/reference/webkit2gtk/stable/struct.MemoryPressureSettings.html)
/// and [`WebContextExtUnix::new_with_memory_pressure_settings`].
#[cfg(gtk)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemoryPressureSettings {
  /// Memory limit, in MB, that the thresholds are fractions of. Must be bigger than 0.
  ///
  /// WebKitGTK's default is the system's RAM size, with a maximum of 3 GB.
  pub memory_limit_mb: Option<u32>,
  /// Fraction of the memory limit from which the conservative policy releases non-critical
  /// memory. Must be bigger than 0, smaller than 1 and smaller than `strict_threshold`.
  ///
  /// WebKitGTK's default is 0.33.
  pub conservative_threshold: Option<f64>,
  /// Fraction of the memory limit from which the strict policy also releases critical memory.
  /// Must be bigger than `conservative_threshold`, smaller than 1 and, when `kill_threshold` is
  /// not 0, smaller than it.
  ///
  /// WebKitGTK's default is 0.5.
  pub strict_threshold: Option<f64>,
  /// Fraction of the memory limit from which the web process is killed. 0 never kills it; any
  /// other value must be bigger than `strict_threshold` and may exceed 1.
  ///
  /// WebKitGTK's default is 0 (never).
  pub kill_threshold: Option<f64>,
  /// Period between two memory measurements. Must be longer than zero.
  ///
  /// WebKitGTK's default is 30 seconds.
  pub poll_interval: Option<Duration>,
}

/// Additional methods on `WebContext` that are specific to Linux.
#[cfg(gtk)]
pub trait WebContextExtUnix: Sized {
  /// Create a new [`WebContext`] whose web processes use the given WebKitGTK memory-pressure
  /// settings.
  ///
  /// WebKitGTK takes these settings only when the `WebKitWebContext` is constructed (its
  /// `memory-pressure-settings` property is construct-only), so they cannot be changed on an
  /// existing context, and they apply to every webview created with this one. They affect the
  /// web processes only, not the network process. Webviews created with
  /// [`WebViewBuilder::with_incognito`](crate::WebViewBuilder::with_incognito) use their own
  /// ephemeral context and keep WebKitGTK's defaults.
  ///
  /// See [`WebContext::new`] for `data_directory`.
  ///
  /// # Example
  ///
  /// Let a web process use half of a 16 GB machine before WebKitGTK's strict policy starts
  /// (with the default limit of 3 GB it starts at 1.5 GB):
  ///
  /// ```no_run
  /// use wry::{MemoryPressureSettings, WebContext, WebContextExtUnix};
  ///
  /// let context = WebContext::new_with_memory_pressure_settings(
  ///   None,
  ///   MemoryPressureSettings {
  ///     memory_limit_mb: Some(8192),
  ///     ..Default::default()
  ///   },
  /// );
  /// ```
  fn new_with_memory_pressure_settings(
    data_directory: Option<PathBuf>,
    memory_pressure_settings: MemoryPressureSettings,
  ) -> Self;
}

#[cfg(gtk)]
impl WebContextExtUnix for WebContext {
  fn new_with_memory_pressure_settings(
    data_directory: Option<PathBuf>,
    memory_pressure_settings: MemoryPressureSettings,
  ) -> Self {
    Self {
      os: WebContextImpl::new_with_memory_pressure_settings(
        data_directory.as_deref(),
        Some(&memory_pressure_settings),
      ),
      data_directory,
      custom_protocols: Default::default(),
    }
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

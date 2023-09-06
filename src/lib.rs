// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Wry is a Cross-platform WebView rendering library.
//!
//! To build a Window with WebView embedded, we could use [`application`] module to create
//! [`EventLoop`] and the window. It's a module that re-exports APIs from [tao]. Then
//! use [`webview`] module to create the [`WebView`] from the [`Window`]. Here's a minimum example
//! showing how to create a hello world window and load the url to Tauri website.
//!
//! ```no_run
//! fn main() -> wry::Result<()> {
//!   use wry::{
//!     application::{
//!       event::{Event, StartCause, WindowEvent},
//!       event_loop::{ControlFlow, EventLoop},
//!       window::WindowBuilder,
//!     },
//!     webview::WebViewBuilder,
//!   };
//!
//!   let event_loop = EventLoop::new();
//!   let window = WindowBuilder::new()
//!     .with_title("Hello World")
//!     .build(&event_loop)?;
//!   let _webview = WebViewBuilder::new(window)?
//!     .with_url("https://tauri.studio")?
//!     .build()?;
//!
//!   event_loop.run(move |event, _, control_flow| {
//!     *control_flow = ControlFlow::Wait;
//!
//!     match event {
//!       Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
//!       Event::WindowEvent {
//!         event: WindowEvent::CloseRequested,
//!         ..
//!       } => *control_flow = ControlFlow::Exit,
//!       _ => (),
//!     }
//!   });
//! }
//! ```
//!
//! ## Feature flags
//!
//! Wry uses a set of feature flags to toggle several advanced features. `file-drop`, `protocol`,
//! are enabled by default.
//!
//! - `tao`: Default windowing crate used by wry. It is re-exported as `application` module.
//! - `winit`: Replace [tao] with [winit] crate. It only supports Windows and macOS.
//! - `file-drop`: Enables [`with_file_drop_handler`] to control the behaviour when there are files
//! interacting with the window. Enabled by default.
//! - `protocol`: Enables [`with_custom_protocol`] to define custom URL scheme for handling tasks like
//! loading assets. Enabled by default.
//!  This feature requires either `libayatana-appindicator` or `libappindicator` package installed.
//!  You can still create those types if you disable it. They just don't create the actual objects.
//! - `devtools`: Enables devtools on release builds. Devtools are always enabled in debug builds.
//! On **macOS**, enabling devtools, requires calling private apis so you should not enable this flag in release
//! build if your app needs to publish to App Store.
//! - `transparent`: Transparent background on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! - `fullscreen`: Fullscreen video and other media on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! - `dox`: Enables this in `package.metadata.docs.rs` section to skip linking some **Linux**
//! libraries and prevent from building documentation on doc.rs fails.
//! - `linux-headers`: Enables headers support of custom protocol request on Linux. Requires
//! webkit2gtk v2.36 or above.
//!
//! [tao]: https://crates.io/crates/tao
//! [`EventLoop`]: crate::application::event_loop::EventLoop
//! [`Window`]: crate::application::window::Window
//! [`WebView`]: crate::webview::WebView
//! [`with_file_drop_handler`]: crate::webview::WebView::with_file_drop_handler
//! [`with_custom_protocol`]: crate::webview::WebView::with_custom_protocol

#![allow(clippy::new_without_default)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unit_cmp)]
#![allow(clippy::upper_case_acronyms)]

#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(any(target_os = "macos", target_os = "ios"))]
#[macro_use]
extern crate objc;

use std::sync::mpsc::{RecvError, SendError};

use crate::application::window::BadIcon;
pub use serde_json::Value;
use url::ParseError;

pub mod application;
pub use http;
pub mod webview;

/// Convenient type alias of Result type for wry.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by wry.
#[non_exhaustive]
#[derive(Error, Debug)]
pub enum Error {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  #[error(transparent)]
  GlibError(#[from] glib::Error),
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  #[error(transparent)]
  GlibBoolError(#[from] glib::BoolError),
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
  ))]
  #[error("Fail to fetch security manager")]
  MissingManager,
  #[error("Failed to initialize the script")]
  InitScriptError,
  #[error("Bad RPC request: {0} ((1))")]
  RpcScriptError(String, String),
  #[error(transparent)]
  NulError(#[from] std::ffi::NulError),
  #[error(transparent)]
  OsError(#[from] crate::application::error::OsError),
  #[error(transparent)]
  ReceiverError(#[from] RecvError),
  #[error(transparent)]
  SenderError(#[from] SendError<String>),
  #[error("Failed to send the message")]
  MessageSender,
  #[error(transparent)]
  Json(#[from] serde_json::Error),
  #[error(transparent)]
  UrlError(#[from] ParseError),
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Icon error: {0}")]
  Icon(#[from] BadIcon),
  #[cfg(target_os = "windows")]
  #[error("WebView2 error: {0}")]
  WebView2Error(webview2_com::Error),
  #[error("Duplicate custom protocol registered: {0}")]
  DuplicateCustomProtocol(String),
  #[error(transparent)]
  HttpError(#[from] http::Error),
  #[error("Infallible error, something went really wrong: {0}")]
  Infallible(#[from] std::convert::Infallible),
  #[cfg(target_os = "android")]
  #[error(transparent)]
  JniError(#[from] tao::platform::android::ndk_glue::jni::errors::Error),
  #[error("Failed to create proxy endpoint")]
  ProxyEndpointCreationFailed,
}

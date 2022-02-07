// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
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
//! and `tray` are enabled by default.
//!
//! - `file-drop`: Enables [`with_file_drop_handler`] to control the behaviour when there are files
//! interacting with the window. Enabled by default.
//! - `protocol`: Enables [`with_custom_protocol`] to define custom URL scheme for handling tasks like
//! loading assets. Enabled by default.
//! - `tray`: Enables system tray and more menu item variants on **Linux**. You can still create
//! those types if you disable it. They just don't create the actual objects. We set this flag
//! because some implementations require more installed packages. Disable this if you don't want
//! to install `libappindicator` package. Enabled by default.
//! - `ayatana`: Enable this if you wish to use more update `libayatana-appindicator` since
//! `libappindicator` is no longer maintained.
//! - `devtool`: Enable devtool on **macOS** requires calling private functions. While it's still
//! enabled in **debug** build on mac, it will require this feature flag to actually enable it in
//! **release** build. Avoid this in release build if your app needs to publish to App Store.
//! - `transparent`: Transparent background on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! - `fullscreen`: Fullscreen video and other media on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! - `dox`: Enables this in `package.metadata.docs.rs` section to skip linking some **Linux**
//! libraries and prevent from building documentation on doc.rs fails.
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

use crate::{
  application::window::BadIcon,
  http::{
    header::{InvalidHeaderName, InvalidHeaderValue},
    method::InvalidMethod,
    status::InvalidStatusCode,
    InvalidUri,
  },
};
pub use serde_json::Value;
use url::ParseError;

pub mod application;
pub mod http;
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
  #[error("Invalid header name: {0}")]
  InvalidHeaderName(#[from] InvalidHeaderName),
  #[error("Invalid header value: {0}")]
  InvalidHeaderValue(#[from] InvalidHeaderValue),
  #[error("Invalid uri: {0}")]
  InvalidUri(#[from] InvalidUri),
  #[error("Invalid status code: {0}")]
  InvalidStatusCode(#[from] InvalidStatusCode),
  #[error("Invalid method: {0}")]
  InvalidMethod(#[from] InvalidMethod),
  #[error("Infallible error, something went really wrong: {0}")]
  Infallible(#[from] std::convert::Infallible),
}

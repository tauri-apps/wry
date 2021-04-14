// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Wry is a Cross-platform WebView rendering library.
//!
//! There are two main ways to build WebView windows: [`Application`] and build by yourself.
//!
//! # Building WebView windows through [`Application`]
//!
//! [`Application`] is the recommended way to build the WebView windows. It provides ergonomic and
//! unified APIs across all platforms. To get started, you simply create an [`Application`] first:
//!
//! ```ignore
//! let application = Application::new()?;
//! ```
//!
//! Once you have your application instance, you can create the WebView window by calling
//! [`Application::add_window`] with [`Attributes`] as the argument to configure the WebView window.
//! If you don't have any preference, you could just set it with `Default::default()`.
//!
//! ```ignore
//! let attributes = Attributes {
//!     url: Some("https://tauri.studio".to_string()),
//!     title: String::from("Hello World!"),
//!     // Initialization scripts can be used to define javascript functions and variables.
//!     initialization_scripts: vec![
//!         String::from("breads = NaN"),
//!         String::from("menacing = 'ゴ'"),
//!     ],
//!     ..Default::default()
//! };
//!
//! let window = app.add_window(attributes)?;
//! ```
//!
//! Run the application with run in the end. This will consume the instance and run the application
//! on current thread.
//!
//! ```ignore
//! application.run();
//! ```
//!
//! # Building WebView windows by yourself
//!
//! If you want to control whole windows creation and events handling, you can use
//! [`WebViewBuilder`] / [`WebView`] under [webview] module to build it all by yourself. You need
//! [winit] for you to build the window across all platforms except Linux. We still need Gtk's
//! library to build the WebView, so it's [gtk-rs] on Linux.
//!
//! ## Feature flags
//!
//! Wry uses a set of feature flags to toggle several advanced features.
//!
//! - `file-drop`: Enable [`FileDropHandler`] to control the behaviour when there are files
//! interacting with the window.
//! - `protocol`: Enable [`CustomProtocol`] to define custom URL scheme for handling tasks like
//! loading assets.
//!
//! ## Debug build
//!
//! Debug profile enables tools like inspector for development or debug usage. Note this will call
//! private APIs on macOS.
//!
//! [winit]: https://crates.io/crates/winit
//! [gtk-rs]: https://crates.io/crates/gtk
//!

#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

use std::sync::mpsc::{RecvError, SendError};

pub use serde_json::Value;
use url::ParseError;
#[cfg(not(target_os = "linux"))]
use winit::window::BadIcon;

#[cfg(feature = "protocol")]
pub use application::CustomProtocol;
#[cfg(not(feature = "protocol"))]
pub(crate) use application::CustomProtocol;
pub use application::{
  Application, ApplicationProxy, Attributes, Event, Icon, Message, WindowEvent, WindowId,
  WindowMessage, WindowProxy, WindowRpcHandler,
};
#[cfg(not(feature = "file-drop"))]
pub(crate) use file_drop::FileDropEvent;
#[cfg(feature = "file-drop")]
pub use file_drop::{FileDropEvent, WindowFileDropHandler};
#[cfg(not(feature = "file-drop"))]
pub(crate) use webview::FileDropHandler;
#[cfg(feature = "file-drop")]
pub use webview::FileDropHandler;
pub(crate) use webview::{RpcHandler, WebView, WebViewBuilder};
pub use webview::{RpcRequest, RpcResponse};

mod application;
mod file_drop;
pub mod webview;

/// Convenient type alias of Result type for wry.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by wry.
#[derive(Error, Debug)]
pub enum Error {
  #[cfg(target_os = "linux")]
  #[error(transparent)]
  GlibError(#[from] glib::Error),
  #[cfg(target_os = "linux")]
  #[error(transparent)]
  GlibBoolError(#[from] glib::BoolError),
  #[cfg(target_os = "linux")]
  #[error("Fail to fetch security manager")]
  MissingManager,
  #[error("Failed to initialize the script")]
  InitScriptError,
  #[error("Bad RPC request: {0} ((1))")]
  RpcScriptError(String, String),
  #[error(transparent)]
  NulError(#[from] std::ffi::NulError),
  #[cfg(not(target_os = "linux"))]
  #[error(transparent)]
  OsError(#[from] winit::error::OsError),
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
  #[error("image error: {0}")]
  Image(#[from] image::ImageError),
  #[cfg(not(target_os = "linux"))]
  #[error("Icon error: {0}")]
  Icon(#[from] BadIcon),
  #[cfg(target_os = "windows")]
  #[cfg(feature = "winrt")]
  #[error(transparent)]
  WindowsError(#[from] windows::Error),
  #[cfg(target_os = "windows")]
  #[cfg(feature = "win32")]
  #[error(transparent)]
  WebView2Error(#[from] webview2::Error),
}

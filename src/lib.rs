// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Wry is a Cross-platform WebView rendering library.
//!
//! To build a Window with WebView embedded, we could use [`application`] module to create
//! [`EventLoop`] and the window. It's a module that re-exports APIs from [winit]. Then
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
//!     *control_flow = ControlFlow::Poll;
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
//! Wry uses a set of feature flags to toggle several advanced features.
//!
//! - `file-drop`: Enable [`with_file_drop_handler`] to control the behaviour when there are files
//! interacting with the window.
//! - `protocol`: Enable [`with_custom_protocol`] to define custom URL scheme for handling tasks like
//! loading assets.
//!
//! ## Debug build
//!
//! Debug profile enables tools like inspector for development or debug usage. Note this will call
//! private APIs on macOS.
//!
//! [winit]: https://crates.io/crates/winit
//! [`EventLoop`]: crate::application::event_loop::EventLoop
//! [`Window`]: crate::application::window::Window
//! [`WebView`]: crate::webview::WebView
//! [`with_file_drop_handler`]: crate::webview::WebView::with_file_drop_handler
//! [`with_custom_protocol`]: crate::webview::WebView::with_custom_protocol

#![allow(clippy::new_without_default)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unit_cmp)]
#![allow(clippy::upper_case_acronyms)]

#[cfg(target_os = "linux")]
#[macro_use]
extern crate bitflags;
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

pub mod application;
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

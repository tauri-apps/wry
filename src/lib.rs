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
//!       Application
//!     },
//!     webview::WebViewBuilder,
//!   };
//!  
//!   let event_loop = EventLoop::new();
//!   let application = Application::new(None);
//!   let window = WindowBuilder::new()
//!     .with_title("Hello World")
//!     .build(&event_loop)?;
//!   let _webview = WebViewBuilder::new(window, &application)?
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
//! `tray`, and `win32` are enabled by default.
//!
//! - `file-drop`: Enables [`with_file_drop_handler`] to control the behaviour when there are files
//! interacting with the window.
//! - `protocol`: Enables [`with_custom_protocol`] to define custom URL scheme for handling tasks like
//! loading assets.
//! - `tray`: Enables system tray and more menu item variants on **Linux**. You can still create
//! those types if you disable it. They just don't create the actual objects. We set this flag
//! because some implementations require more installed packages. Disable this if you don't want
//! to install `libappindicator`, `sourceview`, and `clang` package.
//! - `menu`: Enables menu item variants on **Linux**. You can still create those types if you
//! you disable it. They just don't create the actual objects. We set this flag  because some
//! implementations require more installed packages. Disable this if you don't want to install
//! `sourceview` package.
//! - `win32`: Enables purely Win32 APIs to build the WebView on **Windows**. This makes backward
//! compatibility down to Windows 7 possible.
//! - `dox`: Enables this in `package.metadata.docs.rs` section to skip linking some **Linux**
//! libraries and prevent from building documentation on doc.rs fails.
//!
//! ## Debug build
//!
//! Debug profile enables tools like inspector for development or debug usage. Note this will call
//! private APIs on macOS.
//!
//! [tao]: https://crates.io/crates/tao
//! [`EventLoop`]: crate::application::event_loop::EventLoop
//! [`Window`]: crate::application::window::Window
//! [`WebView`]: crate::webview::WebView
//! [`with_file_drop_handler`]: crate::webview::WebView::with_file_drop_handler
//! [`with_custom_protocol`]: crate::webview::WebView::with_custom_protocol

#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![allow(clippy::new_without_default)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::too_many_arguments)]
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

#[cfg(not(target_os = "linux"))]
use crate::application::window::BadIcon;
pub use serde_json::Value;
use url::ParseError;

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

use crate::{
  application::{
    window::{Window, WindowBuilder},
    Application,
  },
  webview::{Dispatcher, WebViewBuilder},
  webview::{FileDropEvent, RpcRequest, RpcResponse, WebView},
};

use tao::{
  dpi::{Position, Size},
  event_loop::EventLoopWindowTarget,
  menu::Menu,
  window::{Fullscreen, Icon},
};

macro_rules! window_builder {
  (
    $(#[$meta:meta])+
    method => $method:ident,
    original => $original:ident,
    $(
      arg => $arg:ident: $type:path,
      $(generic => $generic:path)?
    )?
  ) => {
    $(#[$meta])+
    #[doc = ""]
    #[doc = "_**Note:** if the [`Builder`] was created with [`Builder::with_window`] then this method will have no effect._"]
    pub fn $method $($(<T: $generic>)?)? (mut self $(, $arg: $type)? ) -> Self {
      if let BuilderWindowBuilder::Builder(builder) = self.window {
        self.window = BuilderWindowBuilder::Builder(builder.$original($($arg)?));
      }

      self
    }
  };
}

/// lol what do i call this
enum BuilderWindowBuilder {
  Window(Window),
  Builder(WindowBuilder),
}

pub struct Builder<'event, Event: 'static> {
  event_loop: &'event EventLoopWindowTarget<Event>,
  window: BuilderWindowBuilder,
  webview: WebViewBuilder,
}

impl<'event, Event: 'static> Builder<'event, Event> {
  pub fn new(event_loop: &'event EventLoopWindowTarget<Event>) -> Self {
    Builder {
      event_loop,
      window: BuilderWindowBuilder::Builder(WindowBuilder::new()),
      webview: WebViewBuilder::new(),
    }
  }

  pub fn with_window(event_loop: &'event EventLoopWindowTarget<Event>, window: Window) -> Self {
    Self {
      event_loop,
      window: BuilderWindowBuilder::Window(window),
      webview: WebViewBuilder::new(),
    }
  }

  window_builder! {
    /// Requests the window to be of specific dimensions.
    ///
    /// See [`WindowBuilder::with_inner_size`] for details.
    method => inner_size,
    original => with_inner_size,
    arg => size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a minimum dimension size for the window.
    ///
    /// See [`WindowBuilder::with_min_inner_size`] for details.
    method => min_inner_size,
    original => with_min_inner_size,
    arg => min_size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a maximum dimension size for the window.
    ///
    /// See [`WindowBuilder::with_max_inner_size`] for details.
    method => max_inner_size,
    original => with_max_inner_size,
    arg => max_size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a desired initial position for the window.
    ///
    /// See [`WindowBuilder::with_position`] for details.
    method => position,
    original => with_position,
    arg => position: T,
    generic => Into<Position>
  }

  window_builder! {
    /// Sets whether the window is resizable or not.
    ///
    /// See [`WindowBuilder::with_resizable`] for details.
    method => resizable,
    original => with_resizable,
    arg => resizable: bool,
  }

  window_builder! {
    /// Requests a specific title for the window.
    ///
    /// See [`WindowBuilder::with_title`] for details.
    method => title,
    original => with_title,
    arg => title: T,
    generic => Into<String>
  }

  window_builder! {
    /// Requests a specific menu for the window.
    ///
    /// See [`WindowBuilder::with_menu`] for details.
    method => menu,
    original => with_menu,
    arg => menu: T,
    generic => Into<Vec<Menu>>
  }

  window_builder! {
    /// Sets the window fullscreen state.
    ///
    /// See [`WindowBuilder::with_fullscreen`] for details.
    method => fullscreen,
    original => with_fullscreen,
    arg => fullscreen: Option<Fullscreen>,
  }

  window_builder! {
    /// Requests maximized mode.
    ///
    /// See [`WindowBuilder::with_maximized`] for details.
    method => maximized,
    original => with_maximized,
    arg => maximized: bool,
  }

  window_builder! {
    /// Sets whether the window will be initially hidden or visible.
    ///
    /// See [`WindowBuilder::with_visible`] for details.
    method => visible,
    original => with_visible,
    arg => visible: bool,
  }

  // todo: this is the only setter that doesn't take a bool and that seems wrong on a builder
  window_builder! {
    /// Sets whether the window will be initially hidden or focus.
    ///
    /// See [`WindowBuilder::with_focus`] for details.
    method => focus,
    original => with_focus,
  }

  window_builder! {
    /// Sets whether the background of the window should be transparent.
    ///
    /// See [`WindowBuilder::with_transparent`] for details.
    method => transparent_window,
    original => with_transparent,
    arg => transparent: bool,
  }

  window_builder! {
    /// Sets whether the window should have a border, a title bar, etc.
    ///
    /// See [`WindowBuilder::with_decorations`] for details.
    method => decorations,
    original => with_decorations,
    arg => decorations: bool,
  }

  window_builder! {
    /// Sets whether or not the window will always be on top of other windows.
    ///
    /// See [`WindowBuilder::with_always_on_top`] for details.
    method => always_on_top,
    original => with_always_on_top,
    arg => always_on_top: bool,
  }

  window_builder! {
    /// Sets the window icon.
    ///
    /// See [`WindowBuilder::with_window_icon`] for details.
    method => window_icon,
    original => with_window_icon,
    arg => window_icon: Option<Icon>,
  }

  /// Whether the [`WebView`] should be transparent.
  ///
  /// See [`WebViewBuilder::with_transparent`] for details.
  pub fn transparent_webview(mut self, transparent: bool) -> Self {
    self.webview = self.webview.with_transparent(transparent);
    self
  }

  /// Set both the [`Window`] and [`WebView`] to be transparent.
  ///
  /// See [`Builder::transparent_window`] and [`Builder::transparent_webview`] for details.
  pub fn transparent(self, transparent: bool) -> Self {
    self
      .transparent_window(transparent)
      .transparent_webview(transparent)
  }

  /// Initialize javascript code when loading new pages.
  ///
  /// See [`WebViewBuilder::with_initialization_script`] for details.
  pub fn initialization_script(mut self, js: &str) -> Self {
    self.webview = self.webview.with_initialization_script(js);
    self
  }

  /// Create a [`Dispatcher`] to send evaluation scripts to the [`WebView`].
  ///
  /// See [`WebViewBuilder::dispatcher`] for details.
  pub fn dispatcher(&self) -> Dispatcher {
    self.webview.dispatcher()
  }

  /// Register custom file loading protocol.
  ///
  /// See [`WebViewBuilder::with_custom_protocol`] for details.
  pub fn custom_protocol<F>(mut self, name: String, handler: F) -> Self
  where
    F: Fn(&Window, &str) -> Result<Vec<u8>> + 'static,
  {
    self.webview = self.webview.with_custom_protocol(name, handler);
    self
  }

  /// Set the RPC handler to Communicate between the host Rust code and Javascript on [`WebView`].
  ///
  /// See [`WebViewBuilder::with_rpc_handler`] for details.
  pub fn rpc_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, RpcRequest) -> Option<RpcResponse> + 'static,
  {
    self.webview = self.webview.with_rpc_handler(handler);
    self
  }

  /// Set a handler closure to process incoming [`FileDropEvent`] of the [`WebView`].
  ///
  /// See [`WebViewBuilder::with_file_drop_handler`] for details.
  pub fn file_drop_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, FileDropEvent) -> bool + 'static,
  {
    self.webview = self.webview.with_file_drop_handler(handler);
    self
  }

  /// The URL to initialize the [`WebView`] with.
  ///
  /// See [`WebViewBuilder::with_url`] for details.
  pub fn url(mut self, url: &str) -> crate::Result<Self> {
    self.webview = self.webview.with_url(url)?;
    Ok(self)
  }

  /// Build the resulting [`WebView`].
  pub fn build(self, application: &Application) -> crate::Result<WebView> {
    let window = match self.window {
      BuilderWindowBuilder::Window(window) => window,
      BuilderWindowBuilder::Builder(builder) => builder.build(self.event_loop)?,
    };

    self.webview.build(window, application)
  }
}

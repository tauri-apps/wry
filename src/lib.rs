// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Wry is a Cross-platform WebView rendering library.
//!
//! The webview requires a running event loop and a window type that implements [`HasWindowHandle`],
//! or a gtk container widget if you need to support X11 and Wayland.
//! You can use a windowing library like [`tao`] or [`winit`].
//!
//! ## Examples
//!
//! This example leverages the [`HasWindowHandle`] and supports Windows, macOS, iOS, Android and Linux (X11 Only).
//! See the following example using [`winit`].
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle};
//! # use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};
//!
//! #[derive(Default)]
//! struct App {
//!   window: Option<Window>,
//!   webview: Option<wry::WebView>,
//! }
//!
//! impl ApplicationHandler for App {
//!   fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!     let window = event_loop.create_window(Window::default_attributes()).unwrap();
//!     let webview = WebViewBuilder::new()
//!       .with_url("https://tauri.app")
//!       .build(&window)
//!       .unwrap();
//!
//!     self.window = Some(window);
//!     self.webview = Some(webview);
//!   }
//!
//!   fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut app = App::default();
//! event_loop.run_app(&mut app).unwrap();
//! ```
//!
//! If you also want to support Wayland too, then we recommend you use [`WebViewBuilderExtUnix::new_gtk`] on Linux.
//! See the following example using [`tao`].
//!
//! ```no_run
//! # use wry::WebViewBuilder;
//! # use tao::{window::WindowBuilder, event_loop::EventLoop};
//! # #[cfg(target_os = "linux")]
//! # use tao::platform::unix::WindowExtUnix;
//! # #[cfg(target_os = "linux")]
//! # use wry::WebViewBuilderExtUnix;
//! let event_loop = EventLoop::new();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//!
//! let builder = WebViewBuilder::new().with_url("https://tauri.app");
//!
//! #[cfg(not(target_os = "linux"))]
//! let webview = builder.build(&window).unwrap();
//! #[cfg(target_os = "linux")]
//! let webview = builder.build_gtk(window.gtk_window()).unwrap();
//! ```
//!
//! ## Child webviews
//!
//! You can use [`WebView::new_as_child`] or [`WebViewBuilder::new_as_child`] to create the webview as a child inside another window. This is supported on
//! macOS, Windows and Linux (X11 Only).
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle, Rect, dpi::*};
//! # use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};
//!
//! #[derive(Default)]
//! struct App {
//!   window: Option<Window>,
//!   webview: Option<wry::WebView>,
//! }
//!
//! impl ApplicationHandler for App {
//!   fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!     let window = event_loop.create_window(Window::default_attributes()).unwrap();
//!     let webview = WebViewBuilder::new()
//!       .with_url("https://tauri.app")
//!       .with_bounds(Rect {
//!         position: LogicalPosition::new(100, 100).into(),
//!         size: LogicalSize::new(200, 200).into(),
//!       })
//!       .build_as_child(&window)
//!       .unwrap();
//!
//!     self.window = Some(window);
//!     self.webview = Some(webview);
//!   }
//!
//!   fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut app = App::default();
//! event_loop.run_app(&mut app).unwrap();
//! ```
//!
//! If you want to support X11 and Wayland at the same time, we recommend using
//! [`WebViewExtUnix::new_gtk`] or [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle, Rect, dpi::*};
//! # use tao::{window::WindowBuilder, event_loop::EventLoop};
//! # #[cfg(target_os = "linux")]
//! # use wry::WebViewBuilderExtUnix;
//! # #[cfg(target_os = "linux")]
//! # use tao::platform::unix::WindowExtUnix;
//! let event_loop = EventLoop::new();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//!
//! let builder = WebViewBuilder::new()
//!   .with_url("https://tauri.app")
//!   .with_bounds(Rect {
//!     position: LogicalPosition::new(100, 100).into(),
//!     size: LogicalSize::new(200, 200).into(),
//!   });
//!
//! #[cfg(not(target_os = "linux"))]
//! let webview = builder.build_as_child(&window).unwrap();
//! #[cfg(target_os = "linux")]
//! let webview = {
//!   # use gtk::prelude::*;
//!   let vbox = window.default_vbox().unwrap(); // tao adds a gtk::Box by default
//!   let fixed = gtk::Fixed::new();
//!   fixed.show_all();
//!   vbox.pack_start(&fixed, true, true, 0);
//!   builder.build_gtk(&fixed).unwrap()
//! };
//! ```
//!
//! ## Platform Considerations
//!
//! Note that on Linux, we use webkit2gtk webviews so if the windowing library doesn't support gtk (as in [`winit`])
//! you'll need to call [`gtk::init`] before creating the webview and then call [`gtk::main_iteration_do`] alongside
//! your windowing library event loop.
//!
//! ```no_run
//! # use wry::{WebViewBuilder, raw_window_handle};
//! # use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::{Window, WindowId}};
//!
//! #[derive(Default)]
//! struct App {
//!   window: Option<Window>,
//!   webview: Option<wry::WebView>,
//! }
//!
//! impl ApplicationHandler for App {
//!   fn resumed(&mut self, event_loop: &ActiveEventLoop) {
//!     let window = event_loop.create_window(Window::default_attributes()).unwrap();
//!     let webview = WebViewBuilder::new()
//!       .with_url("https://tauri.app")
//!       .build(&window)
//!       .unwrap();
//!
//!     self.window = Some(window);
//!     self.webview = Some(webview);
//!   }
//!
//!   fn window_event(&mut self, _event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {}
//!
//!   // Advance GTK event loop <!----- IMPORTANT
//!   fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
//!     #[cfg(target_os = "linux")]
//!     while gtk::events_pending() {
//!       gtk::main_iteration_do(false);
//!     }
//!   }
//! }
//!
//! let event_loop = EventLoop::new().unwrap();
//! let mut app = App::default();
//! event_loop.run_app(&mut app).unwrap();
//! ```
//!
//! ## Android
//!
//! In order for `wry` to be able to create webviews on Android, there is a few requirements that your application needs to uphold:
//! 1. You need to set a few environment variables that will be used to generate the necessary kotlin
//! files that you need to include in your Android application for wry to function properly.
//!     - `WRY_ANDROID_PACKAGE`: which is the reversed domain name of your android project and the app name in snake_case, for example, `com.wry.example.wry_app`
//!     - `WRY_ANDROID_LIBRARY`: for example, if your cargo project has a lib name `wry_app`, it will generate `libwry_app.so` so you se this env var to `wry_app`
//!     - `WRY_ANDROID_KOTLIN_FILES_OUT_DIR`: for example, `path/to/app/src/main/kotlin/com/wry/example`
//! 2. Your main Android Activity needs to inherit `AppCompatActivity`, preferably it should use the generated `WryActivity` or inherit it.
//! 3. Your Rust app needs to call `wry::android_setup` function to setup the necessary logic to be able to create webviews later on.
//! 4. Your Rust app needs to call `wry::android_binding!` macro to setup the JNI functions that will be called by `WryActivity` and various other places.
//!
//! It is recommended to use [`tao`](https://docs.rs/tao/latest/tao/) crate as it provides maximum compatibility with `wry`
//!
//! ```
//! #[cfg(target_os = "android")]
//! {
//!   tao::android_binding!(
//!       com_example,
//!       wry_app,
//!       WryActivity,
//!       wry::android_setup, // pass the wry::android_setup function to tao which will invoke when the event loop is created
//!       _start_app
//!   );
//!   wry::android_binding!(com_example, ttt);
//! }
//! ```
//!
//! If this feels overwhelming, you can just use the preconfigured template from [`cargo-mobile2`](https://github.com/tauri-apps/cargo-mobile2).
//!
//! For more inforamtion, checkout [MOBILE.md](https://github.com/tauri-apps/wry/blob/dev/MOBILE.md).
//!
//! ## Feature flags
//!
//! Wry uses a set of feature flags to toggle several advanced features.
//!
//! - `os-webview` (default): Enables the default WebView framework on the platform. This must be enabled
//! for the crate to work. This feature was added in preparation of other ports like cef and servo.
//! - `protocol` (default): Enables [`WebViewBuilder::with_custom_protocol`] to define custom URL scheme for handling tasks like
//! loading assets.
//! - `drag-drop` (default): Enables [`WebViewBuilder::with_drag_drop_handler`] to control the behaviour when there are files
//! interacting with the window.
//! - `devtools`: Enables devtools on release builds. Devtools are always enabled in debug builds.
//! On **macOS**, enabling devtools, requires calling private apis so you should not enable this flag in release
//! build if your app needs to publish to App Store.
//! - `transparent`: Transparent background on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! - `fullscreen`: Fullscreen video and other media on **macOS** requires calling private functions.
//! Avoid this in release build if your app needs to publish to App Store.
//! libraries and prevent from building documentation on doc.rs fails.
//! - `linux-body`: Enables body support of custom protocol request on Linux. Requires
//! webkit2gtk v2.40 or above.
//! - `tracing`: enables [`tracing`] for `evaluate_script`, `ipc_handler` and `custom_protocols.
//!
//! [`tao`]: https://docs.rs/tao
//! [`winit`]: https://docs.rs/winit
//! [`tracing`]: https://docs.rs/tracing

#![allow(clippy::new_without_default)]
#![allow(clippy::default_constructed_unit_structs)]
#![allow(clippy::type_complexity)]
#![cfg_attr(docsrs, feature(doc_cfg))]

// #[cfg(any(target_os = "macos", target_os = "ios"))]
// #[macro_use]
// extern crate objc;

mod error;
mod proxy;
#[cfg(any(target_os = "macos", target_os = "android", target_os = "ios"))]
mod util;
mod web_context;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "android")]
pub use crate::android::android_setup;
#[cfg(target_os = "android")]
pub mod prelude {
  pub use crate::android::{binding::*, dispatch, find_class, Context};
  pub use tao_macros::{android_fn, generate_package_name};
}
#[cfg(target_os = "android")]
pub use android::JniHandle;
#[cfg(target_os = "android")]
use android::*;

#[cfg(gtk)]
pub(crate) mod webkitgtk;
/// Re-exported [raw-window-handle](https://docs.rs/raw-window-handle/latest/raw_window_handle/) crate.
pub use raw_window_handle;
use raw_window_handle::HasWindowHandle;
#[cfg(gtk)]
use webkitgtk::*;

#[cfg(any(target_os = "macos", target_os = "ios"))]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSWindow;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use objc2_web_kit::WKUserContentController;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) mod wkwebview;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use wkwebview::*;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub use wkwebview::{PrintMargin, PrintOptions, WryWebView};

#[cfg(target_os = "windows")]
pub(crate) mod webview2;
#[cfg(target_os = "windows")]
pub use self::webview2::ScrollBarStyle;
#[cfg(target_os = "windows")]
use self::webview2::*;
#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;

use std::{borrow::Cow, collections::HashMap, path::PathBuf, rc::Rc};

use http::{Request, Response};

pub use cookie;
pub use dpi;
pub use error::*;
pub use http;
pub use proxy::{ProxyConfig, ProxyEndpoint};
pub use web_context::WebContext;

/// A rectangular region.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
  /// Rect position.
  pub position: dpi::Position,
  /// Rect size.
  pub size: dpi::Size,
}

impl Default for Rect {
  fn default() -> Self {
    Self {
      position: dpi::LogicalPosition::new(0, 0).into(),
      size: dpi::LogicalSize::new(0, 0).into(),
    }
  }
}

/// Resolves a custom protocol [`Request`] asynchronously.
///
/// See [`WebViewBuilder::with_asynchronous_custom_protocol`] for more information.
pub struct RequestAsyncResponder {
  pub(crate) responder: Box<dyn FnOnce(Response<Cow<'static, [u8]>>)>,
}

// SAFETY: even though the webview bindings do not indicate the responder is Send,
// it actually is and we need it in order to let the user do the protocol computation
// on a separate thread or async task.
unsafe impl Send for RequestAsyncResponder {}

impl RequestAsyncResponder {
  /// Resolves the request with the given response.
  pub fn respond<T: Into<Cow<'static, [u8]>>>(self, response: Response<T>) {
    let (parts, body) = response.into_parts();
    (self.responder)(Response::from_parts(parts, body.into()))
  }
}

/// An id for a webview
pub type WebViewId<'a> = &'a str;

pub struct WebViewAttributes<'a> {
  /// An id that will be passed when this webview makes requests in certain callbacks.
  pub id: Option<WebViewId<'a>>,

  /// Web context to be shared with this webview.
  pub context: Option<&'a mut WebContext>,

  /// Whether the WebView should have a custom user-agent.
  pub user_agent: Option<String>,

  /// Whether the WebView window should be visible.
  pub visible: bool,

  /// Whether the WebView should be transparent.
  ///
  /// ## Platform-specific:
  ///
  /// **Windows 7**: Not supported.
  pub transparent: bool,

  /// Specify the webview background color. This will be ignored if `transparent` is set to `true`.
  ///
  /// The color uses the RGBA format.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS / iOS**: Not implemented.
  /// - **Windows**:
  ///   - On Windows 7, transparency is not supported and the alpha value will be ignored.
  ///   - On Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
  pub background_color: Option<RGBA>,

  /// Whether load the provided URL to [`WebView`].
  ///
  /// ## Note
  ///
  /// Data URLs are not supported, use [`html`](Self::html) option instead.
  pub url: Option<String>,

  /// Headers used when loading the requested [`url`](Self::url).
  pub headers: Option<http::HeaderMap>,

  /// Whether page zooming by hotkeys is enabled
  ///
  /// ## Platform-specific
  ///
  /// **macOS / Linux / Android / iOS**: Unsupported
  pub zoom_hotkeys_enabled: bool,

  /// Whether load the provided html string to [`WebView`].
  /// This will be ignored if the `url` is provided.
  ///
  /// # Warning
  ///
  /// The Page loaded from html string will have `null` origin.
  ///
  /// ## PLatform-specific:
  ///
  /// - **Windows:** the string can not be larger than 2 MB (2 * 1024 * 1024 bytes) in total size
  pub html: Option<String>,

  /// A list of initialization javascript scripts to run when loading new pages.
  /// When webview load a new page, this initialization code will be executed.
  /// It is guaranteed that code is executed before `window.onload`.
  ///
  /// Second parameter represents if script should be added to main frame only or sub frames also.
  /// `true` for main frame only, `false` for sub frames.
  ///
  /// ## Platform-specific
  ///
  /// - **Android:** The Android WebView does not provide an API for initialization scripts,
  /// so we prepend them to each HTML head. They are only implemented on custom protocol URLs.
  pub initialization_scripts: Vec<(String, bool)>,

  /// A list of custom loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes an Id ([WebViewId]), [Request] and [RequestAsyncResponder] as arguments and returns a [Response].
  ///
  /// # Note
  ///
  /// If using a shared [WebContext], make sure custom protocols were not already registered on that web context on Linux.
  ///
  /// # Warning
  ///
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS, iOS and Linux: `<scheme_name>://<path>` (so it will be `wry://path/to/page/`).
  /// - Windows and Android: `http://<scheme_name>.<path>` by default (so it will be `http://wry.path/to/page). To use `https` instead of `http`, use [`WebViewBuilderExtWindows::with_https_scheme`] and [`WebViewBuilderExtAndroid::with_https_scheme`].
  ///
  /// # Reading assets on mobile
  ///
  /// - Android: Android has `assets` and `resource` path finder to
  /// locate your files in those directories. For more information, see [Loading in-app content](https://developer.android.com/guide/webapps/load-local-content) page.
  /// - iOS: To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  pub custom_protocols:
    HashMap<String, Box<dyn Fn(WebViewId, Request<Vec<u8>>, RequestAsyncResponder)>>,

  /// The IPC handler to receive the message from Javascript on webview
  /// using `window.ipc.postMessage("insert_message_here")` to host Rust code.
  pub ipc_handler: Option<Box<dyn Fn(Request<String>)>>,

  /// A handler closure to process incoming [`DragDropEvent`] of the webview.
  ///
  /// # Blocking OS Default Behavior
  /// Return `true` in the callback to block the OS' default behavior.
  ///
  /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
  /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
  #[cfg(feature = "drag-drop")]
  #[cfg_attr(docsrs, doc(cfg(feature = "drag-drop")))]
  pub drag_drop_handler: Option<Box<dyn Fn(DragDropEvent) -> bool>>,
  #[cfg(not(feature = "drag-drop"))]
  drag_drop_handler: Option<Box<dyn Fn(DragDropEvent) -> bool>>,

  /// A navigation handler to decide if incoming url is allowed to navigate.
  ///
  /// The closure take a `String` parameter as url and returns a `bool` to determine whether the navigation should happen.
  /// `true` allows to navigate and `false` does not.
  pub navigation_handler: Option<Box<dyn Fn(String) -> bool>>,

  /// A download started handler to manage incoming downloads.
  ///
  /// The closure takes two parameters, the first is a `String` representing the url being downloaded from and and the
  /// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
  /// parameter can be used to set the download location by assigning a new path to it, the assigned path _must_ be
  /// absolute. The closure returns a `bool` to allow or deny the download.
  pub download_started_handler: Option<Box<dyn FnMut(String, &mut PathBuf) -> bool + 'static>>,

  /// A download completion handler to manage downloads that have finished.
  ///
  /// The closure is fired when the download completes, whether it was successful or not.
  /// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
  /// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
  /// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
  /// did not succeed, and may instead indicate some other failure, always check the third parameter if you need to
  /// know if the download succeeded.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS**: The second parameter indicating the path the file was saved to, is always empty,
  /// due to API limitations.
  pub download_completed_handler: Option<Rc<dyn Fn(String, Option<PathBuf>, bool) + 'static>>,

  /// A new window handler to decide if incoming url is allowed to open in a new window.
  ///
  /// The closure take a `String` parameter as url and return `bool` to determine whether the window should open.
  /// `true` allows to open and `false` does not.
  pub new_window_req_handler: Option<Box<dyn Fn(String) -> bool>>,

  /// Enables clipboard access for the page rendered on **Linux** and **Windows**.
  ///
  /// macOS doesn't provide such method and is always enabled by default. But your app will still need to add menu
  /// item accelerators to use the clipboard shortcuts.
  pub clipboard: bool,

  /// Enable web inspector which is usually called browser devtools.
  ///
  /// Note this only enables devtools to the webview. To open it, you can call
  /// [`WebView::open_devtools`], or right click the page and open it from the context menu.
  ///
  /// ## Platform-specific
  ///
  /// - macOS: This will call private functions on **macOS**. It is enabled in **debug** builds,
  /// but requires `devtools` feature flag to actually enable it in **release** builds.
  /// - Android: Open `chrome://inspect/#devices` in Chrome to get the devtools window. Wry's `WebView` devtools API isn't supported on Android.
  /// - iOS: Open Safari > Develop > [Your Device Name] > [Your WebView] to get the devtools window.
  pub devtools: bool,

  /// Whether clicking an inactive window also clicks through to the webview. Default is `false`.
  ///
  /// ## Platform-specific
  ///
  /// This configuration only impacts macOS.
  pub accept_first_mouse: bool,

  /// Indicates whether horizontal swipe gestures trigger backward and forward page navigation.
  ///
  /// ## Platform-specific:
  ///
  /// - Windows: Setting to `false` does nothing on WebView2 Runtime version before 92.0.902.0,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10902-prerelease
  ///
  /// - **Android / iOS:** Unsupported.
  pub back_forward_navigation_gestures: bool,

  /// Set a handler closure to process the change of the webview's document title.
  pub document_title_changed_handler: Option<Box<dyn Fn(String)>>,

  /// Run the WebView with incognito mode. Note that WebContext will be ingored if incognito is
  /// enabled.
  ///
  /// ## Platform-specific:
  ///
  /// - Windows: Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039
  /// - **Android:** Unsupported yet.
  pub incognito: bool,

  /// Whether all media can be played without user interaction.
  pub autoplay: bool,

  /// Set a handler closure to process page load events.
  pub on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,

  /// Set a proxy configuration for the webview. Supports HTTP CONNECT and SOCKSv5 proxies
  ///
  /// - **macOS**: Requires macOS 14.0+ and the `mac-proxy` feature flag to be enabled.
  /// - **Android / iOS:** Not supported.
  pub proxy_config: Option<ProxyConfig>,

  /// Whether the webview should be focused when created.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS / Android / iOS:** Unsupported.
  pub focused: bool,

  /// The webview bounds. Defaults to `x: 0, y: 0, width: 200, height: 200`.
  /// This is only effective if the webview was created by [`WebView::new_as_child`] or [`WebViewBuilder::new_as_child`]
  /// or on Linux, if was created by [`WebViewExtUnix::new_gtk`] or [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
  pub bounds: Option<Rect>,
}

impl<'a> Default for WebViewAttributes<'a> {
  fn default() -> Self {
    Self {
      id: Default::default(),
      context: None,
      user_agent: None,
      visible: true,
      transparent: false,
      background_color: None,
      url: None,
      headers: None,
      html: None,
      initialization_scripts: Default::default(),
      custom_protocols: Default::default(),
      ipc_handler: None,
      drag_drop_handler: None,
      navigation_handler: None,
      download_started_handler: None,
      download_completed_handler: None,
      new_window_req_handler: None,
      clipboard: false,
      #[cfg(debug_assertions)]
      devtools: true,
      #[cfg(not(debug_assertions))]
      devtools: false,
      zoom_hotkeys_enabled: false,
      accept_first_mouse: false,
      back_forward_navigation_gestures: false,
      document_title_changed_handler: None,
      incognito: false,
      autoplay: true,
      on_page_load_handler: None,
      proxy_config: None,
      focused: true,
      bounds: Some(Rect {
        position: dpi::LogicalPosition::new(0, 0).into(),
        size: dpi::LogicalSize::new(200, 200).into(),
      }),
    }
  }
}

struct WebviewBuilderParts<'a> {
  attrs: WebViewAttributes<'a>,
  platform_specific: PlatformSpecificWebViewAttributes,
}

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] provides ability to setup initialization before web engine starts.
pub struct WebViewBuilder<'a> {
  inner: Result<WebviewBuilderParts<'a>>,
}

impl<'a> WebViewBuilder<'a> {
  /// Create a new [`WebViewBuilder`].
  pub fn new() -> Self {
    Self {
      inner: Ok(WebviewBuilderParts {
        attrs: WebViewAttributes::default(),
        #[allow(clippy::default_constructed_unit_structs)]
        platform_specific: PlatformSpecificWebViewAttributes::default(),
      }),
    }
  }

  /// Create a new [`WebViewBuilder`] with a web context that can be shared with multiple [`WebView`]s.
  pub fn with_web_context(web_context: &'a mut WebContext) -> Self {
    let mut attrs = WebViewAttributes::default();
    attrs.context = Some(web_context);

    Self {
      inner: Ok(WebviewBuilderParts {
        attrs,
        #[allow(clippy::default_constructed_unit_structs)]
        platform_specific: PlatformSpecificWebViewAttributes::default(),
      }),
    }
  }

  /// Create a new [`WebViewBuilder`] with the given [`WebViewAttributes`]
  pub fn with_attributes(attrs: WebViewAttributes<'a>) -> Self {
    Self {
      inner: Ok(WebviewBuilderParts {
        attrs,
        #[allow(clippy::default_constructed_unit_structs)]
        platform_specific: PlatformSpecificWebViewAttributes::default(),
      }),
    }
  }

  fn and_then<F>(self, func: F) -> Self
  where
    F: FnOnce(WebviewBuilderParts<'a>) -> Result<WebviewBuilderParts<'a>>,
  {
    Self {
      inner: self.inner.and_then(func),
    }
  }

  /// Set an id that will be passed when this webview makes requests in certain callbacks.
  pub fn with_id(self, id: WebViewId<'a>) -> Self {
    self.and_then(|mut b| {
      b.attrs.id = Some(id);
      Ok(b)
    })
  }

  /// Indicates whether horizontal swipe gestures trigger backward and forward page navigation.
  ///
  /// ## Platform-specific:
  ///
  /// - **Android / iOS:** Unsupported.
  pub fn with_back_forward_navigation_gestures(self, gesture: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.back_forward_navigation_gestures = gesture;
      Ok(b)
    })
  }

  /// Sets whether the WebView should be transparent.
  ///
  /// ## Platform-specific:
  ///
  /// **Windows 7**: Not supported.
  pub fn with_transparent(self, transparent: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.transparent = transparent;
      Ok(b)
    })
  }

  /// Specify the webview background color. This will be ignored if `transparent` is set to `true`.
  ///
  /// The color uses the RGBA format.
  ///
  /// ## Platfrom-specific:
  ///
  /// - **macOS / iOS**: Not implemented.
  /// - **Windows**:
  ///   - on Windows 7, transparency is not supported and the alpha value will be ignored.
  ///   - on Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
  pub fn with_background_color(self, background_color: RGBA) -> Self {
    self.and_then(|mut b| {
      b.attrs.background_color = Some(background_color);
      Ok(b)
    })
  }

  /// Sets whether the WebView should be visible or not.
  pub fn with_visible(self, visible: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.visible = visible;
      Ok(b)
    })
  }

  /// Sets whether all media can be played without user interaction.
  pub fn with_autoplay(self, autoplay: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.autoplay = autoplay;
      Ok(b)
    })
  }

  /// Initialize javascript code when loading new pages. When webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  ///
  /// ## Example
  /// ```ignore
  /// let webview = WebViewBuilder::new()
  ///   .with_initialization_script("console.log('Running inside main frame only')")
  ///   .with_url("https://tauri.app")
  ///   .build(&window)
  ///   .unwrap();
  /// ```
  ///
  /// ## Platform-specific
  ///
  /// - **Android:** When [addDocumentStartJavaScript] is not supported,
  /// we prepend them to each HTML head (implementation only supported on custom protocol URLs).
  /// For remote URLs, we use [onPageStarted] which is not guaranteed to run before other scripts.
  ///
  /// [addDocumentStartJavaScript]: https://developer.android.com/reference/androidx/webkit/WebViewCompat#addDocumentStartJavaScript(android.webkit.WebView,java.lang.String,java.util.Set%3Cjava.lang.String%3E)
  /// [onPageStarted]: https://developer.android.com/reference/android/webkit/WebViewClient#onPageStarted(android.webkit.WebView,%20java.lang.String,%20android.graphics.Bitmap)
  pub fn with_initialization_script(self, js: &str) -> Self {
    self.with_initialization_script_for_main_only(js, true)
  }

  /// Same as [`with_initialization_script`](Self::with_initialization_script) but with option to inject into main frame only or sub frames.
  ///
  /// ## Example
  /// ```ignore
  /// let webview = WebViewBuilder::new()
  ///   .with_initialization_script_for_main_only("console.log('Running inside main frame only')", true)
  ///   .with_initialization_script_for_main_only("console.log('Running  main frame and sub frames')", false)
  ///   .with_url("https://tauri.app")
  ///   .build(&window)
  ///   .unwrap();
  /// ```
  pub fn with_initialization_script_for_main_only(self, js: &str, main_only: bool) -> Self {
    self.and_then(|mut b| {
      if !js.is_empty() {
        b.attrs
          .initialization_scripts
          .push((js.to_string(), main_only));
      }
      Ok(b)
    })
  }

  /// Register custom loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes a [Request] and returns a [Response]
  ///
  /// When registering a custom protocol with the same name, only the last regisered one will be used.
  ///
  /// # Warning
  ///
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS, iOS and Linux: `<scheme_name>://<path>` (so it will be `wry://path/to/page).
  /// - Windows and Android: `http://<scheme_name>.<path>` by default (so it will be `http://wry.path/to/page`). To use `https` instead of `http`, use [`WebViewBuilderExtWindows::with_https_scheme`] and [`WebViewBuilderExtAndroid::with_https_scheme`].
  ///
  /// # Reading assets on mobile
  ///
  /// - Android: For loading content from the `assets` folder (which is copied to the Andorid apk) please
  /// use the function [`with_asset_loader`] from [`WebViewBuilderExtAndroid`] instead.
  /// This function on Android can only be used to serve assets you can embed in the binary or are
  /// elsewhere in Android (provided the app has appropriate access), but not from the `assets`
  /// folder which lives within the apk. For the cases where this can be used, it works the same as in macOS and Linux.
  /// - iOS: To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  #[cfg(feature = "protocol")]
  pub fn with_custom_protocol<F>(self, name: String, handler: F) -> Self
  where
    F: Fn(WebViewId, Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> + 'static,
  {
    self.and_then(|mut b| {
      #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
      ))]
      if let Some(context) = &mut b.attrs.context {
        context.register_custom_protocol(name.clone())?;
      }

      if b.attrs.custom_protocols.iter().any(|(n, _)| n == &name) {
        return Err(Error::DuplicateCustomProtocol(name));
      }

      b.attrs.custom_protocols.insert(
        name,
        Box::new(move |id, request, responder| {
          let http_response = handler(id, request);
          responder.respond(http_response);
        }),
      );

      Ok(b)
    })
  }

  /// Same as [`Self::with_custom_protocol`] but with an asynchronous responder.
  ///
  /// When registering a custom protocol with the same name, only the last regisered one will be used.
  ///
  /// # Examples
  ///
  /// ```no_run
  /// use wry::{WebViewBuilder, raw_window_handle};
  /// WebViewBuilder::new()
  ///   .with_asynchronous_custom_protocol("wry".into(), |_webview_id, request, responder| {
  ///     // here you can use a tokio task, thread pool or anything
  ///     // to do heavy computation to resolve your request
  ///     // e.g. downloading files, opening the camera...
  ///     std::thread::spawn(move || {
  ///       std::thread::sleep(std::time::Duration::from_secs(2));
  ///       responder.respond(http::Response::builder().body(Vec::new()).unwrap());
  ///     });
  ///   });
  /// ```
  #[cfg(feature = "protocol")]
  pub fn with_asynchronous_custom_protocol<F>(self, name: String, handler: F) -> Self
  where
    F: Fn(WebViewId, Request<Vec<u8>>, RequestAsyncResponder) + 'static,
  {
    self.and_then(|mut b| {
      #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
      ))]
      if let Some(context) = &mut b.attrs.context {
        context.register_custom_protocol(name.clone())?;
      }

      if b.attrs.custom_protocols.iter().any(|(n, _)| n == &name) {
        return Err(Error::DuplicateCustomProtocol(name));
      }

      b.attrs.custom_protocols.insert(name, Box::new(handler));

      Ok(b)
    })
  }

  /// Set the IPC handler to receive the message from Javascript on webview
  /// using `window.ipc.postMessage("insert_message_here")` to host Rust code.
  ///
  /// ## Platform-specific
  ///
  /// - **Linux / Android**: The request URL is not supported on iframes and the main frame URL is used instead.
  pub fn with_ipc_handler<F>(self, handler: F) -> Self
  where
    F: Fn(Request<String>) + 'static,
  {
    self.and_then(|mut b| {
      b.attrs.ipc_handler = Some(Box::new(handler));
      Ok(b)
    })
  }

  /// Set a handler closure to process incoming [`DragDropEvent`] of the webview.
  ///
  /// # Blocking OS Default Behavior
  /// Return `true` in the callback to block the OS' default behavior.
  ///
  /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
  /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
  #[cfg(feature = "drag-drop")]
  #[cfg_attr(docsrs, doc(cfg(feature = "drag-drop")))]
  pub fn with_drag_drop_handler<F>(self, handler: F) -> Self
  where
    F: Fn(DragDropEvent) -> bool + 'static,
  {
    self.and_then(|mut b| {
      b.attrs.drag_drop_handler = Some(Box::new(handler));
      Ok(b)
    })
  }

  /// Load the provided URL with given headers when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
  /// The provided URL must be valid.
  ///
  /// ## Note
  ///
  /// Data URLs are not supported, use [`html`](Self::with_html) option instead.
  pub fn with_url_and_headers(self, url: impl Into<String>, headers: http::HeaderMap) -> Self {
    self.and_then(|mut b| {
      b.attrs.url = Some(url.into());
      b.attrs.headers = Some(headers);
      Ok(b)
    })
  }

  /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
  /// The provided URL must be valid.
  ///
  /// ## Note
  ///
  /// Data URLs are not supported, use [`html`](Self::with_html) option instead.
  pub fn with_url(self, url: impl Into<String>) -> Self {
    self.and_then(|mut b| {
      b.attrs.url = Some(url.into());
      b.attrs.headers = None;
      Ok(b)
    })
  }

  /// Set headers used when loading the requested [`url`](Self::with_url).
  pub fn with_headers(self, headers: http::HeaderMap) -> Self {
    self.and_then(|mut b| {
      b.attrs.headers = Some(headers);
      Ok(b)
    })
  }

  /// Load the provided HTML string when the builder calling [`WebViewBuilder::build`] to create the [`WebView`].
  /// This will be ignored if `url` is provided.
  ///
  /// # Warning
  ///
  /// The Page loaded from html string will have `null` origin.
  ///
  /// ## PLatform-specific:
  ///
  /// - **Windows:** the string can not be larger than 2 MB (2 * 1024 * 1024 bytes) in total size
  pub fn with_html(self, html: impl Into<String>) -> Self {
    self.and_then(|mut b| {
      b.attrs.html = Some(html.into());
      Ok(b)
    })
  }

  /// Set a custom [user-agent](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/User-Agent) for the WebView.
  ///
  /// ## Platform-specific
  ///
  /// - Windows: Requires WebView2 Runtime version 86.0.616.0 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10790-prerelease
  pub fn with_user_agent(self, user_agent: impl Into<String>) -> Self {
    self.and_then(|mut b| {
      b.attrs.user_agent = Some(user_agent.into());
      Ok(b)
    })
  }

  /// Enable or disable web inspector which is usually called devtools.
  ///
  /// Note this only enables devtools to the webview. To open it, you can call
  /// [`WebView::open_devtools`], or right click the page and open it from the context menu.
  ///
  /// ## Platform-specific
  ///
  /// - macOS: This will call private functions on **macOS**. It is enabled in **debug** builds,
  /// but requires `devtools` feature flag to actually enable it in **release** builds.
  /// - Android: Open `chrome://inspect/#devices` in Chrome to get the devtools window. Wry's `WebView` devtools API isn't supported on Android.
  /// - iOS: Open Safari > Develop > [Your Device Name] > [Your WebView] to get the devtools window.
  pub fn with_devtools(self, devtools: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.devtools = devtools;
      Ok(b)
    })
  }

  /// Whether page zooming by hotkeys or gestures is enabled
  ///
  /// ## Platform-specific
  ///
  /// - Windows: Setting to `false` can't disable pinch zoom on WebView2 Runtime version before 91.0.865.0,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10865-prerelease
  ///
  /// - **macOS / Linux / Android / iOS**: Unsupported
  pub fn with_hotkeys_zoom(self, zoom: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.zoom_hotkeys_enabled = zoom;
      Ok(b)
    })
  }

  /// Set a navigation handler to decide if incoming url is allowed to navigate.
  ///
  /// The closure take a `String` parameter as url and returns a `bool` to determine whether the navigation should happen.
  /// `true` allows to navigate and `false` does not.
  pub fn with_navigation_handler(self, callback: impl Fn(String) -> bool + 'static) -> Self {
    self.and_then(|mut b| {
      b.attrs.navigation_handler = Some(Box::new(callback));
      Ok(b)
    })
  }

  /// Set a download started handler to manage incoming downloads.
  ///
  //// The closure takes two parameters, the first is a `String` representing the url being downloaded from and and the
  /// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
  /// parameter can be used to set the download location by assigning a new path to it, the assigned path _must_ be
  /// absolute. The closure returns a `bool` to allow or deny the download.
  pub fn with_download_started_handler(
    self,
    download_started_handler: impl FnMut(String, &mut PathBuf) -> bool + 'static,
  ) -> Self {
    self.and_then(|mut b| {
      b.attrs.download_started_handler = Some(Box::new(download_started_handler));
      Ok(b)
    })
  }

  /// Sets a download completion handler to manage downloads that have finished.
  ///
  /// The closure is fired when the download completes, whether it was successful or not.
  /// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
  /// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
  /// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
  /// did not succeed, and may instead indicate some other failure, always check the third parameter if you need to
  /// know if the download succeeded.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS**: The second parameter indicating the path the file was saved to, is always empty,
  /// due to API limitations.
  pub fn with_download_completed_handler(
    self,
    download_completed_handler: impl Fn(String, Option<PathBuf>, bool) + 'static,
  ) -> Self {
    self.and_then(|mut b| {
      b.attrs.download_completed_handler = Some(Rc::new(download_completed_handler));
      Ok(b)
    })
  }

  /// Enables clipboard access for the page rendered on **Linux** and **Windows**.
  ///
  /// macOS doesn't provide such method and is always enabled by default. But your app will still need to add menu
  /// item accelerators to use the clipboard shortcuts.
  pub fn with_clipboard(self, clipboard: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.clipboard = clipboard;
      Ok(b)
    })
  }

  /// Set a new window request handler to decide if incoming url is allowed to be opened.
  ///
  /// The closure take a `String` parameter as url and return `bool` to determine whether the window should open.
  /// `true` allows to open and `false` does not.
  pub fn with_new_window_req_handler(self, callback: impl Fn(String) -> bool + 'static) -> Self {
    self.and_then(|mut b| {
      b.attrs.new_window_req_handler = Some(Box::new(callback));
      Ok(b)
    })
  }

  /// Sets whether clicking an inactive window also clicks through to the webview. Default is `false`.
  ///
  /// ## Platform-specific
  ///
  /// This configuration only impacts macOS.
  pub fn with_accept_first_mouse(self, accept_first_mouse: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.accept_first_mouse = accept_first_mouse;
      Ok(b)
    })
  }

  /// Set a handler closure to process the change of the webview's document title.
  pub fn with_document_title_changed_handler(self, callback: impl Fn(String) + 'static) -> Self {
    self.and_then(|mut b| {
      b.attrs.document_title_changed_handler = Some(Box::new(callback));
      Ok(b)
    })
  }

  /// Run the WebView with incognito mode. Note that WebContext will be ingored if incognito is
  /// enabled.
  ///
  /// ## Platform-specific:
  ///
  /// - Windows: Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039
  /// - **Android:** Unsupported yet.
  pub fn with_incognito(self, incognito: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.incognito = incognito;
      Ok(b)
    })
  }

  /// Set a handler to process page loading events.
  pub fn with_on_page_load_handler(
    self,
    handler: impl Fn(PageLoadEvent, String) + 'static,
  ) -> Self {
    self.and_then(|mut b| {
      b.attrs.on_page_load_handler = Some(Box::new(handler));
      Ok(b)
    })
  }

  /// Set a proxy configuration for the webview.
  ///
  /// - **macOS**: Requires macOS 14.0+ and the `mac-proxy` feature flag to be enabled. Supports HTTP CONNECT and SOCKSv5 proxies.
  /// - **Windows / Linux**: Supports HTTP CONNECT and SOCKSv5 proxies.
  /// - **Android / iOS:** Not supported.
  pub fn with_proxy_config(self, configuration: ProxyConfig) -> Self {
    self.and_then(|mut b| {
      b.attrs.proxy_config = Some(configuration);
      Ok(b)
    })
  }

  /// Set whether the webview should be focused when created.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS / Android / iOS:** Unsupported.
  pub fn with_focused(self, focused: bool) -> Self {
    self.and_then(|mut b| {
      b.attrs.focused = focused;
      Ok(b)
    })
  }

  /// Specify the webview position relative to its parent if it will be created as a child
  /// or if created using [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
  ///
  /// Defaults to `x: 0, y: 0, width: 200, height: 200`.
  pub fn with_bounds(self, bounds: Rect) -> Self {
    self.and_then(|mut b| {
      b.attrs.bounds = Some(bounds);
      Ok(b)
    })
  }

  /// Consume the builder and create the [`WebView`] from a type that implements [`HasWindowHandle`].
  ///
  /// # Platform-specific:
  ///
  /// - **Linux**: Only X11 is supported, if you want to support Wayland too, use [`WebViewBuilderExtUnix::new_gtk`].
  ///
  ///   Although this methods only needs an X11 window handle, we use webkit2gtk, so you still need to initialize gtk
  ///   by callling [`gtk::init`] and advance its loop alongside your event loop using [`gtk::main_iteration_do`].
  ///   Checkout the [Platform Considerations](https://docs.rs/wry/latest/wry/#platform-considerations) section in the crate root documentation.
  /// - **Windows**: The webview will auto-resize when the passed handle is resized.
  /// - **Linux (X11)**: Unlike macOS and Windows, the webview will not auto-resize and you'll need to call [`WebView::set_bounds`] manually.
  ///
  /// # Panics:
  ///
  /// - Panics if the provided handle was not supported or invalid.
  /// - Panics on Linux, if [`gtk::init`] was not called in this thread.
  pub fn build<W: HasWindowHandle>(self, window: &'a W) -> Result<WebView> {
    let parts = self.inner?;

    InnerWebView::new(window, parts.attrs, parts.platform_specific)
      .map(|webview| WebView { webview })
  }

  /// Consume the builder and create the [`WebView`] as a child window inside the provided [`HasWindowHandle`].
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: This will create the webview as a child window of the `parent` window.
  /// - **macOS**: This will create the webview as a `NSView` subview of the `parent` window's
  /// content view.
  /// - **Linux**: This will create the webview as a child window of the `parent` window. Only X11
  /// is supported. This method won't work on Wayland.
  ///
  ///   Although this methods only needs an X11 window handle, you use webkit2gtk, so you still need to initialize gtk
  ///   by callling [`gtk::init`] and advance its loop alongside your event loop using [`gtk::main_iteration_do`].
  ///   Checkout the [Platform Considerations](https://docs.rs/wry/latest/wry/#platform-considerations) section in the crate root documentation.
  ///
  ///   If you want to support child webviews on X11 and Wayland at the same time,
  ///   we recommend using [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
  /// - **Android/iOS:** Unsupported.
  ///
  /// # Panics:
  ///
  /// - Panics if the provided handle was not support or invalid.
  /// - Panics on Linux, if [`gtk::init`] was not called in this thread.
  pub fn build_as_child<W: HasWindowHandle>(self, window: &'a W) -> Result<WebView> {
    let parts = self.inner?;

    InnerWebView::new_as_child(window, parts.attrs, parts.platform_specific)
      .map(|webview| WebView { webview })
  }
}

#[cfg(any(target_os = "macos", target_os = "ios",))]
#[derive(Clone, Default)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  data_store_identifier: Option<[u8; 16]>,
  traffic_light_inset: Option<dpi::Position>,
}

#[cfg(any(target_os = "macos", target_os = "ios",))]
pub trait WebViewBuilderExtDarwin {
  /// Initialize the WebView with a custom data store identifier.
  /// Can be used as a replacement for data_directory not being available in WKWebView.
  ///
  /// - **macOS / iOS**: Available on macOS >= 14 and iOS >= 17
  fn with_data_store_identifier(self, identifier: [u8; 16]) -> Self;
  /// Move the window controls to the specified position.
  /// Normally this is handled by the Window but because `WebViewBuilder::build()` overwrites the window's NSView the controls will flicker on resizing.
  /// Note: This method has no effects if the WebView is injected via `WebViewBuilder::build_as_child();` and there should be no flickers.
  /// Warning: Do not use this if your chosen window library does not support traffic light insets.
  /// Warning: Only use this in **decorated** windows with a **hidden titlebar**!
  fn with_traffic_light_inset<P: Into<dpi::Position>>(self, position: P) -> Self;
}

#[cfg(any(target_os = "macos", target_os = "ios",))]
impl WebViewBuilderExtDarwin for WebViewBuilder<'_> {
  fn with_data_store_identifier(self, identifier: [u8; 16]) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.data_store_identifier = Some(identifier);
      Ok(b)
    })
  }

  fn with_traffic_light_inset<P: Into<dpi::Position>>(self, position: P) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.traffic_light_inset = Some(position.into());
      Ok(b)
    })
  }
}

#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  additional_browser_args: Option<String>,
  browser_accelerator_keys: bool,
  theme: Option<Theme>,
  use_https: bool,
  scroll_bar_style: ScrollBarStyle,
  browser_extensions_enabled: bool,
  extension_path: Option<PathBuf>,
}

#[cfg(windows)]
impl Default for PlatformSpecificWebViewAttributes {
  fn default() -> Self {
    Self {
      additional_browser_args: None,
      browser_accelerator_keys: true, // This is WebView2's default behavior
      theme: None,
      use_https: false, // To match macOS & Linux behavior in the context of mixed content.
      scroll_bar_style: ScrollBarStyle::default(),
      browser_extensions_enabled: false,
      extension_path: None,
    }
  }
}

#[cfg(windows)]
pub trait WebViewBuilderExtWindows {
  /// Pass additional args to WebView2 upon creating the webview.
  ///
  /// ## Warning
  ///
  /// - Webview instances with different browser arguments must also have different [data directories](struct.WebContext.html#method.new).
  /// - By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
  /// `--autoplay-policy=no-user-gesture-required` if autoplay is enabled
  /// and `--proxy-server=<scheme>://<host>:<port>` if a proxy is set.
  /// so if you use this method, you have to add these arguments yourself if you want to keep the same behavior.
  fn with_additional_browser_args<S: Into<String>>(self, additional_args: S) -> Self;

  /// Determines whether browser-specific accelerator keys are enabled. When this setting is set to
  /// `false`, it disables all accelerator keys that access features specific to a web browser.
  /// The default value is `true`. See the following link to know more details.
  ///
  /// Setting to `false` does nothing on WebView2 Runtime version before 92.0.902.0,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10824-prerelease
  ///
  /// <https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings#arebrowseracceleratorkeysenabled>
  fn with_browser_accelerator_keys(self, enabled: bool) -> Self;

  /// Specifies the theme of webview2. This affects things like `prefers-color-scheme`.
  ///
  /// Defaults to [`Theme::Auto`] which will follow the OS defaults.
  ///
  /// Requires WebView2 Runtime version 101.0.1210.39 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039
  fn with_theme(self, theme: Theme) -> Self;

  /// Determines whether the custom protocols should use `https://<scheme>.path/to/page` instead of the default `http://<scheme>.path/to/page`.
  ///
  /// Using a `http` scheme will allow mixed content when trying to fetch `http` endpoints
  /// and is therefore less secure but will match the behavior of the `<scheme>://path/to/page` protocols used on macOS and Linux.
  ///
  /// The default value is `false`.
  fn with_https_scheme(self, enabled: bool) -> Self;

  /// Specifies the native scrollbar style to use with webview2.
  /// CSS styles that modify the scrollbar are applied on top of the native appearance configured here.
  ///
  /// Defaults to [`ScrollbarStyle::Default`] which is the browser default used by Microsoft Edge.
  ///
  /// Requires WebView2 Runtime version 125.0.2535.41 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/?tabs=dotnetcsharp#10253541
  fn with_scroll_bar_style(self, style: ScrollBarStyle) -> Self;

  /// Determines whether the ability to install and enable extensions is enabled.
  ///
  /// By default, extensions are disabled.
  ///
  /// Requires WebView2 Runtime version 1.0.2210.55 or higher, does nothing on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10221055
  fn with_browser_extensions_enabled(self, enabled: bool) -> Self;

  /// Set the path from which to load extensions from. Extensions stored in this path should be unpacked.
  ///
  /// Does nothing if browser extensions are disabled. See [`with_browser_extensions_enabled`](Self::with_browser_extensions_enabled)
  fn with_extensions_path(self, path: impl Into<PathBuf>) -> Self;
}

#[cfg(windows)]
impl WebViewBuilderExtWindows for WebViewBuilder<'_> {
  fn with_additional_browser_args<S: Into<String>>(self, additional_args: S) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.additional_browser_args = Some(additional_args.into());
      Ok(b)
    })
  }

  fn with_browser_accelerator_keys(self, enabled: bool) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.browser_accelerator_keys = enabled;
      Ok(b)
    })
  }

  fn with_theme(self, theme: Theme) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.theme = Some(theme);
      Ok(b)
    })
  }

  fn with_https_scheme(self, enabled: bool) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.use_https = enabled;
      Ok(b)
    })
  }

  fn with_scroll_bar_style(self, style: ScrollBarStyle) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.scroll_bar_style = style;
      Ok(b)
    })
  }

  fn with_browser_extensions_enabled(self, enabled: bool) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.browser_extensions_enabled = enabled;
      Ok(b)
    })
  }

  fn with_extensions_path(self, path: impl Into<PathBuf>) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.extension_path = Some(path.into());
      Ok(b)
    })
  }
}

#[cfg(target_os = "android")]
#[derive(Default)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  on_webview_created:
    Option<Box<dyn Fn(prelude::Context) -> std::result::Result<(), jni::errors::Error> + Send>>,
  with_asset_loader: bool,
  asset_loader_domain: Option<String>,
  https_scheme: bool,
}

#[cfg(target_os = "android")]
pub trait WebViewBuilderExtAndroid {
  fn on_webview_created<
    F: Fn(prelude::Context<'_, '_>) -> std::result::Result<(), jni::errors::Error> + Send + 'static,
  >(
    self,
    f: F,
  ) -> Self;

  /// Use [WebViewAssetLoader](https://developer.android.com/reference/kotlin/androidx/webkit/WebViewAssetLoader)
  /// to load assets from Android's `asset` folder when using `with_url` as `<protocol>://assets/` (e.g.:
  /// `wry://assets/index.html`). Note that this registers a custom protocol with the provided
  /// String, similar to [`with_custom_protocol`], but also sets the WebViewAssetLoader with the
  /// necessary domain (which is fixed as `<protocol>.assets`). This cannot be used in conjunction
  /// to `with_custom_protocol` for Android, as it changes the way in which requests are handled.
  #[cfg(feature = "protocol")]
  fn with_asset_loader(self, protocol: String) -> Self;

  /// Determines whether the custom protocols should use `https://<scheme>.localhost` instead of the default `http://<scheme>.localhost`.
  ///
  /// Using a `http` scheme will allow mixed content when trying to fetch `http` endpoints
  /// and is therefore less secure but will match the behavior of the `<scheme>://localhost` protocols used on macOS and Linux.
  ///
  /// The default value is `false`.
  fn with_https_scheme(self, enabled: bool) -> Self;
}

#[cfg(target_os = "android")]
impl WebViewBuilderExtAndroid for WebViewBuilder<'_> {
  fn on_webview_created<
    F: Fn(prelude::Context<'_, '_>) -> std::result::Result<(), jni::errors::Error> + Send + 'static,
  >(
    self,
    f: F,
  ) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.on_webview_created = Some(Box::new(f));
      Ok(b)
    })
  }

  #[cfg(feature = "protocol")]
  fn with_asset_loader(self, protocol: String) -> Self {
    // register custom protocol with empty Response return,
    // this is necessary due to the need of fixing a domain
    // in WebViewAssetLoader.
    self.and_then(|mut b| {
      b.attrs.custom_protocols.insert(
        protocol.clone(),
        Box::new(|_, _, api| {
          api.respond(Response::builder().body(Vec::new()).unwrap());
        }),
      );
      b.platform_specific.with_asset_loader = true;
      b.platform_specific.asset_loader_domain = Some(format!("{}.assets", protocol));
      Ok(b)
    })
  }

  fn with_https_scheme(self, enabled: bool) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.https_scheme = enabled;
      Ok(b)
    })
  }
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
#[derive(Default)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  extension_path: Option<PathBuf>,
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
pub trait WebViewBuilderExtUnix<'a> {
  /// Consume the builder and create the webview inside a GTK container widget, such as GTK window.
  ///
  /// - If the container is [`gtk::Box`], it is added using [`Box::pack_start(webview, true, true, 0)`](gtk::prelude::BoxExt::pack_start).
  /// - If the container is [`gtk::Fixed`], its [size request](gtk::prelude::WidgetExt::set_size_request) will be set using the (width, height) bounds passed in
  ///   and will be added to the container using [`Fixed::put`](gtk::prelude::FixedExt::put) using the (x, y) bounds passed in.
  /// - For all other containers, it will be added using [`gtk::prelude::ContainerExt::add`]
  ///
  /// # Panics:
  ///
  /// - Panics if [`gtk::init`] was not called in this thread.
  fn build_gtk<W>(self, widget: &'a W) -> Result<WebView>
  where
    W: gtk::prelude::IsA<gtk::Container>;

  /// Set the path from which to load extensions from.
  fn with_extensions_path(self, path: impl Into<PathBuf>) -> Self;
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
impl<'a> WebViewBuilderExtUnix<'a> for WebViewBuilder<'a> {
  fn build_gtk<W>(self, widget: &'a W) -> Result<WebView>
  where
    W: gtk::prelude::IsA<gtk::Container>,
  {
    let parts = self.inner?;

    InnerWebView::new_gtk(widget, parts.attrs, parts.platform_specific)
      .map(|webview| WebView { webview })
  }

  fn with_extensions_path(self, path: impl Into<PathBuf>) -> Self {
    self.and_then(|mut b| {
      b.platform_specific.extension_path = Some(path.into());
      Ok(b)
    })
  }
}

/// The fundamental type to present a [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebView`] presents the actual WebView window and let you still able to perform actions on it.
pub struct WebView {
  webview: InnerWebView,
}

impl WebView {
  /// Create a [`WebView`] from from a type that implements [`HasWindowHandle`].
  /// Note that calling this directly loses
  /// abilities to initialize scripts, add ipc handler, and many more before starting WebView. To
  /// benefit from above features, create a [`WebViewBuilder`] instead.
  ///
  /// # Platform-specific:
  ///
  /// - **Linux**: Only X11 is supported, if you want to support Wayland too, use [`WebViewExtUnix::new_gtk`].
  ///
  ///   Although this methods only needs an X11 window handle, you use webkit2gtk, so you still need to initialize gtk
  ///   by callling [`gtk::init`] and advance its loop alongside your event loop using [`gtk::main_iteration_do`].
  ///   Checkout the [Platform Considerations](https://docs.rs/wry/latest/wry/#platform-considerations) section in the crate root documentation.
  /// - **macOS / Windows**: The webview will auto-resize when the passed handle is resized.
  /// - **Linux (X11)**: Unlike macOS and Windows, the webview will not auto-resize and you'll need to call [`WebView::set_bounds`] manually.
  ///
  /// # Panics:
  ///
  /// - Panics if the provided handle was not supported or invalid.
  /// - Panics on Linux, if [`gtk::init`] was not called in this thread.
  pub fn new(window: &impl HasWindowHandle, attrs: WebViewAttributes) -> Result<Self> {
    WebViewBuilder::with_attributes(attrs).build(window)
  }

  /// Create [`WebViewBuilder`] as a child window inside the provided [`HasWindowHandle`].
  ///
  /// ## Platform-specific
  ///
  /// - **Windows**: This will create the webview as a child window of the `parent` window.
  /// - **macOS**: This will create the webview as a `NSView` subview of the `parent` window's
  /// content view.
  /// - **Linux**: This will create the webview as a child window of the `parent` window. Only X11
  /// is supported. This method won't work on Wayland.
  ///
  ///   Although this methods only needs an X11 window handle, you use webkit2gtk, so you still need to initialize gtk
  ///   by callling [`gtk::init`] and advance its loop alongside your event loop using [`gtk::main_iteration_do`].
  ///   Checkout the [Platform Considerations](https://docs.rs/wry/latest/wry/#platform-considerations) section in the crate root documentation.
  ///
  ///   If you want to support child webviews on X11 and Wayland at the same time,
  ///   we recommend using [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
  /// - **Android/iOS:** Unsupported.
  ///
  /// # Panics:
  ///
  /// - Panics if the provided handle was not support or invalid.
  /// - Panics on Linux, if [`gtk::init`] was not called in this thread.
  pub fn new_as_child(parent: &impl HasWindowHandle, attrs: WebViewAttributes) -> Result<Self> {
    WebViewBuilder::with_attributes(attrs).build_as_child(parent)
  }

  /// Returns the id of this webview.
  pub fn id(&self) -> WebViewId {
    self.webview.id()
  }

  /// Get the current url of the webview
  pub fn url(&self) -> Result<String> {
    self.webview.url()
  }

  /// Evaluate and run javascript code.
  pub fn evaluate_script(&self, js: &str) -> Result<()> {
    self
      .webview
      .eval(js, None::<Box<dyn Fn(String) + Send + 'static>>)
  }

  /// Evaluate and run javascript code with callback function. The evaluation result will be
  /// serialized into a JSON string and passed to the callback function.
  ///
  /// Exception is ignored because of the limitation on windows. You can catch it yourself and return as string as a workaround.
  ///
  /// - ** Android:** Not implemented yet.
  pub fn evaluate_script_with_callback(
    &self,
    js: &str,
    callback: impl Fn(String) + Send + 'static,
  ) -> Result<()> {
    self.webview.eval(js, Some(callback))
  }

  /// Launch print modal for the webview content.
  pub fn print(&self) -> Result<()> {
    self.webview.print()
  }

  /// Get a list of cookies for specific url.
  pub fn cookies_for_url(&self, url: &str) -> Result<Vec<cookie::Cookie<'static>>> {
    self.webview.cookies_for_url(url)
  }

  /// Get the list of cookies.
  ///
  /// ## Platform-specific
  ///
  /// - **Android**: Unsupported, always returns an empty [`Vec`].
  pub fn cookies(&self) -> Result<Vec<cookie::Cookie<'static>>> {
    self.webview.cookies()
  }

  /// Open the web inspector which is usually called dev tool.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    self.webview.open_devtools()
  }

  /// Close the web inspector which is usually called dev tool.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows / Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    self.webview.close_devtools()
  }

  /// Gets the devtool window's current visibility state.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows / Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    self.webview.is_devtools_open()
  }

  /// Set the webview zoom level
  ///
  /// ## Platform-specific:
  ///
  /// - **Android**: Not supported.
  /// - **macOS**: available on macOS 11+ only.
  /// - **iOS**: available on iOS 14+ only.
  pub fn zoom(&self, scale_factor: f64) -> Result<()> {
    self.webview.zoom(scale_factor)
  }

  /// Specify the webview background color.
  ///
  /// The color uses the RGBA format.
  ///
  /// ## Platfrom-specific:
  ///
  /// - **macOS / iOS**: Not implemented.
  /// - **Windows**:
  ///   - On Windows 7, transparency is not supported and the alpha value will be ignored.
  ///   - On Windows higher than 7: translucent colors are not supported so any alpha value other than `0` will be replaced by `255`
  pub fn set_background_color(&self, background_color: RGBA) -> Result<()> {
    self.webview.set_background_color(background_color)
  }

  /// Navigate to the specified url
  pub fn load_url(&self, url: &str) -> Result<()> {
    self.webview.load_url(url)
  }

  /// Navigate to the specified url using the specified headers
  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) -> Result<()> {
    self.webview.load_url_with_headers(url, headers)
  }

  /// Load html content into the webview
  pub fn load_html(&self, html: &str) -> Result<()> {
    self.webview.load_html(html)
  }

  /// Clear all browsing data
  pub fn clear_all_browsing_data(&self) -> Result<()> {
    self.webview.clear_all_browsing_data()
  }

  pub fn bounds(&self) -> Result<Rect> {
    self.webview.bounds()
  }

  /// Set the webview bounds.
  ///
  /// This is only effective if the webview was created as a child
  /// or created using [`WebViewBuilderExtUnix::new_gtk`] with [`gtk::Fixed`].
  pub fn set_bounds(&self, bounds: Rect) -> Result<()> {
    self.webview.set_bounds(bounds)
  }

  /// Shows or hides the webview.
  pub fn set_visible(&self, visible: bool) -> Result<()> {
    self.webview.set_visible(visible)
  }

  /// Try moving focus to the webview.
  pub fn focus(&self) -> Result<()> {
    self.webview.focus()
  }

  /// Try moving focus away from the webview back to the parent window.
  ///
  /// ## Platform-specific:
  ///
  /// - **Android**: Not implemented.
  pub fn focus_parent(&self) -> Result<()> {
    self.webview.focus_parent()
  }
}

/// An event describing drag and drop operations on the webview.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum DragDropEvent {
  /// A drag operation has entered the webview.
  Enter {
    /// List of paths that are being dragged onto the webview.
    paths: Vec<PathBuf>,
    /// Position of the drag operation, relative to the webview top-left corner.
    position: (i32, i32),
  },
  /// A drag operation is moving over the window.
  Over {
    /// Position of the drag operation, relative to the webview top-left corner.
    position: (i32, i32),
  },
  /// The file(s) have been dropped onto the window.
  Drop {
    /// List of paths that are being dropped onto the window.
    paths: Vec<PathBuf>,
    /// Position of the drag operation, relative to the webview top-left corner.
    position: (i32, i32),
  },
  /// The drag operation has been cancelled or left the window.
  Leave,
}

/// Get WebView/Webkit version on current platform.
pub fn webview_version() -> Result<String> {
  platform_webview_version()
}

/// The [memory usage target level][1]. There are two levels 'Low' and 'Normal' and the default
/// level is 'Normal'. When the application is going inactive, setting the level to 'Low' can
/// significantly reduce the application's memory consumption.
///
/// [1]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
#[cfg(target_os = "windows")]
#[non_exhaustive]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemoryUsageLevel {
  /// The 'Normal' memory usage. Applications should set this level when they are becoming active.
  #[default]
  Normal,
  /// The 'Low' memory usage. Applications can reduce memory comsumption by setting this level when
  /// they are becoming inactive.
  Low,
}

/// Additional methods on `WebView` that are specific to Windows.
#[cfg(target_os = "windows")]
pub trait WebViewExtWindows {
  /// Returns WebView2 Controller
  fn controller(&self) -> ICoreWebView2Controller;

  /// Changes the webview2 theme.
  ///
  /// Requires WebView2 Runtime version 101.0.1210.39 or higher, returns error on older versions,
  /// see https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/archive?tabs=dotnetcsharp#10121039
  fn set_theme(&self, theme: Theme) -> Result<()>;

  /// Sets the [memory usage target level][1].
  ///
  /// When to best use this mode depends on the app in question. Most commonly it's called when
  /// the app's visiblity state changes.
  ///
  /// Please read the [guide for WebView2][2] for more details.
  ///
  /// This method uses a WebView2 API added in Runtime version 114.0.1823.32. When it is used in
  /// an older Runtime version, it does nothing.
  ///
  /// [1]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel
  /// [2]: https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2.memoryusagetargetlevel?view=webview2-dotnet-1.0.2088.41#remarks
  fn set_memory_usage_level(&self, level: MemoryUsageLevel) -> Result<()>;

  /// Attaches this webview to the given HWND and removes it from the current one.
  fn reparent(&self, hwnd: isize) -> Result<()>;
}

#[cfg(target_os = "windows")]
impl WebViewExtWindows for WebView {
  fn controller(&self) -> ICoreWebView2Controller {
    self.webview.controller.clone()
  }

  fn set_theme(&self, theme: Theme) -> Result<()> {
    self.webview.set_theme(theme)
  }

  fn set_memory_usage_level(&self, level: MemoryUsageLevel) -> Result<()> {
    self.webview.set_memory_usage_level(level)
  }

  fn reparent(&self, hwnd: isize) -> Result<()> {
    self.webview.reparent(hwnd)
  }
}

/// Additional methods on `WebView` that are specific to Linux.
#[cfg(gtk)]
pub trait WebViewExtUnix: Sized {
  /// Create the webview inside a GTK container widget, such as GTK window.
  ///
  /// - If the container is [`gtk::Box`], it is added using [`Box::pack_start(webview, true, true, 0)`](gtk::prelude::BoxExt::pack_start).
  /// - If the container is [`gtk::Fixed`], its [size request](gtk::prelude::WidgetExt::set_size_request) will be set using the (width, height) bounds passed in
  ///   and will be added to the container using [`Fixed::put`](gtk::prelude::FixedExt::put) using the (x, y) bounds passed in.
  /// - For all other containers, it will be added using [`gtk::prelude::ContainerExt::add`]
  ///
  /// # Panics:
  ///
  /// - Panics if [`gtk::init`] was not called in this thread.
  fn new_gtk<W>(widget: &W) -> Result<Self>
  where
    W: gtk::prelude::IsA<gtk::Container>;

  /// Returns Webkit2gtk Webview handle
  fn webview(&self) -> webkit2gtk::WebView;

  /// Attaches this webview to the given Widget and removes it from the current one.
  fn reparent<W>(&self, widget: &W) -> Result<()>
  where
    W: gtk::prelude::IsA<gtk::Container>;
}

#[cfg(gtk)]
impl WebViewExtUnix for WebView {
  fn new_gtk<W>(widget: &W) -> Result<Self>
  where
    W: gtk::prelude::IsA<gtk::Container>,
  {
    WebViewBuilder::new().build_gtk(widget)
  }

  fn webview(&self) -> webkit2gtk::WebView {
    self.webview.webview.clone()
  }

  fn reparent<W>(&self, widget: &W) -> Result<()>
  where
    W: gtk::prelude::IsA<gtk::Container>,
  {
    self.webview.reparent(widget)
  }
}

/// Additional methods on `WebView` that are specific to macOS.
#[cfg(target_os = "macos")]
pub trait WebViewExtMacOS {
  /// Returns WKWebView handle
  fn webview(&self) -> Retained<WryWebView>;
  /// Returns WKWebView manager [(userContentController)](https://developer.apple.com/documentation/webkit/wkscriptmessagehandler/1396222-usercontentcontroller) handle
  fn manager(&self) -> Retained<WKUserContentController>;
  /// Returns NSWindow associated with the WKWebView webview
  fn ns_window(&self) -> Retained<NSWindow>;
  /// Attaches this webview to the given NSWindow and removes it from the current one.
  fn reparent(&self, window: *mut NSWindow) -> Result<()>;
  // Prints with extra options
  fn print_with_options(&self, options: &PrintOptions) -> Result<()>;
  /// Move the window controls to the specified position.
  /// Normally this is handled by the Window but because `WebViewBuilder::build()` overwrites the window's NSView the controls will flicker on resizing.
  /// Note: This method has no effects if the WebView is injected via `WebViewBuilder::build_as_child();` and there should be no flickers.
  /// Warning: Do not use this if your chosen window library does not support traffic light insets.
  /// Warning: Only use this in **decorated** windows with a **hidden titlebar**!
  fn set_traffic_light_inset<P: Into<dpi::Position>>(&self, position: P) -> Result<()>;
}

#[cfg(target_os = "macos")]
impl WebViewExtMacOS for WebView {
  fn webview(&self) -> Retained<WryWebView> {
    self.webview.webview.clone()
  }

  fn manager(&self) -> Retained<WKUserContentController> {
    self.webview.manager.clone()
  }

  fn ns_window(&self) -> Retained<NSWindow> {
    self.webview.webview.window().unwrap().clone()
  }

  fn reparent(&self, window: *mut NSWindow) -> Result<()> {
    self.webview.reparent(window)
  }

  fn print_with_options(&self, options: &PrintOptions) -> Result<()> {
    self.webview.print_with_options(options)
  }

  fn set_traffic_light_inset<P: Into<dpi::Position>>(&self, position: P) -> Result<()> {
    self.webview.set_traffic_light_inset(position.into())
  }
}

/// Additional methods on `WebView` that are specific to iOS.
#[cfg(target_os = "ios")]
pub trait WebViewExtIOS {
  /// Returns WKWebView handle
  fn webview(&self) -> Retained<WryWebView>;
  /// Returns WKWebView manager [(userContentController)](https://developer.apple.com/documentation/webkit/wkscriptmessagehandler/1396222-usercontentcontroller) handle
  fn manager(&self) -> Retained<WKUserContentController>;
}

#[cfg(target_os = "ios")]
impl WebViewExtIOS for WebView {
  fn webview(&self) -> Retained<WryWebView> {
    self.webview.webview.clone()
  }

  fn manager(&self) -> Retained<WKUserContentController> {
    self.webview.manager.clone()
  }
}

#[cfg(target_os = "android")]
/// Additional methods on `WebView` that are specific to Android
pub trait WebViewExtAndroid {
  fn handle(&self) -> JniHandle;
}

#[cfg(target_os = "android")]
impl WebViewExtAndroid for WebView {
  fn handle(&self) -> JniHandle {
    JniHandle
  }
}

/// WebView theme.
#[derive(Debug, Clone, Copy)]
pub enum Theme {
  /// Dark
  Dark,
  /// Light
  Light,
  /// System preference
  Auto,
}

/// Type alias for a color in the RGBA format.
///
/// Each value can be 0..255 inclusive.
pub type RGBA = (u8, u8, u8, u8);

/// Type of of page loading event
pub enum PageLoadEvent {
  /// Indicates that the content of the page has started loading
  Started,
  /// Indicates that the page content has finished loading
  Finished,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[cfg_attr(miri, ignore)]
  fn should_get_webview_version() {
    if let Err(error) = webview_version() {
      panic!("{}", error);
    }
  }
}

// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! [`WebView`] struct and associated types.

mod web_context;

pub use web_context::WebContext;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "android")]
use android::*;
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
pub(crate) mod webkitgtk;
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
use webkitgtk::*;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) mod wkwebview;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use wkwebview::*;
#[cfg(target_os = "windows")]
pub(crate) mod webview2;
#[cfg(target_os = "windows")]
use self::webview2::*;
use crate::Result;
#[cfg(target_os = "android")]
use jni::{
  objects::{JClass, JObject},
  sys::jobject,
  JNIEnv,
};
#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
#[cfg(target_os = "windows")]
use windows::{Win32::Foundation::HWND, Win32::UI::WindowsAndMessaging::DestroyWindow};

use std::{path::PathBuf, rc::Rc};

use url::Url;

#[cfg(target_os = "windows")]
use crate::application::platform::windows::WindowExtWindows;
use crate::application::{dpi::PhysicalSize, window::Window};

use crate::http::{Request as HttpRequest, Response as HttpResponse};

pub struct WebViewAttributes {
  /// Whether the WebView should have a custom user-agent.
  pub user_agent: Option<String>,
  /// Whether the WebView window should be visible.
  pub visible: bool,
  /// Whether the WebView should be transparent. Not supported on Windows 7.
  pub transparent: bool,
  /// Whether load the provided URL to [`WebView`].
  pub url: Option<Url>,
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
  /// The loaded from html string will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS: `http://localhost`
  /// - Linux: `http://localhost`
  /// - Windows: `null`
  /// - Android: Not supported
  /// - iOS: Not supported
  pub html: Option<String>,
  /// Initialize javascript code when loading new pages. When webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  pub initialization_scripts: Vec<String>,
  /// Register custom file loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes a url string slice, and returns a two item tuple of a vector of
  /// bytes which is the content and a mimetype string of the content.
  ///
  /// # Warning
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS: `<scheme_name>://<path>` (so it will be `wry://examples` in `custom_protocol` example)
  /// - Linux: Though it's same as macOS, there's a [bug] that Origin header in the request will be
  /// empty. So the only way to pass the server is setting `Access-Control-Allow-Origin: *`.
  /// - Windows: `https://<scheme_name>.<path>` (so it will be `https://wry.examples` in `custom_protocol` example)
  /// - Android: Custom protocol on Android is fixed to `https://tauri.wry/` due to its design and
  /// our approach to use it. On Android, We only handle the scheme name and ignore the closure. So
  /// when you load the url like `wry://assets/index.html`, it will become
  /// `https://tauri.wry/assets/index.html`. Android has `assets` and `resource` path finder to
  /// locate your files in those directories. For more information, see [Loading in-app content](https://developer.android.com/guide/webapps/load-local-content) page.
  /// - iOS: Same as macOS. To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  ///
  /// [bug]: https://bugs.webkit.org/show_bug.cgi?id=229034
  pub custom_protocols: Vec<(String, Box<dyn Fn(&HttpRequest) -> Result<HttpResponse>>)>,
  /// Set the IPC handler to receive the message from Javascript on webview to host Rust code.
  /// The message sent from webview should call `window.ipc.postMessage("insert_message_here");`.
  ///
  /// Both functions return promises but `notify()` resolves immediately.
  pub ipc_handler: Option<Box<dyn Fn(&Window, String)>>,
  /// Set a handler closure to process incoming [`FileDropEvent`] of the webview.
  ///
  /// # Blocking OS Default Behavior
  /// Return `true` in the callback to block the OS' default behavior of handling a file drop.
  ///
  /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
  /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
  #[cfg(feature = "file-drop")]
  pub file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
  #[cfg(not(feature = "file-drop"))]
  file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,

  /// Set a navigation handler to decide if incoming url is allowed to navigate.
  ///
  /// The closure take a `String` parameter as url and return `bool` to determine the url. True is
  /// allow to nivagate and false is not.
  pub navigation_handler: Option<Box<dyn Fn(String) -> bool>>,

  /// Enables clipboard access for the page rendered on **Linux** and **Windows**.
  ///
  /// macOS doesn't provide such method and is always enabled by default. But you still need to add menu
  /// item accelerators to use shortcuts.
  pub clipboard: bool,

  /// Enable web inspector which is usually called dev tool.
  ///
  /// Note this only enables dev tool to the webview. To open it, you can call
  /// [`WebView::open_devtools`], or right click the page and open it from the context menu.
  ///
  /// ## Platform-specific
  ///
  /// - macOS: This will call private functions on **macOS**. It's still enabled if set in **debug** build on mac,
  /// but requires `devtools` feature flag to actually enable it in **release** build.
  /// - Android: Open `chrome://inspect/#devices` in Chrome to get the devtools window. Wry's `WebView` devtools API isn't supported on Android.
  /// - iOS: Open Safari > Develop > [Your Device Name] > [Your WebView] to get the devtools window.
  pub devtools: bool,
}

impl Default for WebViewAttributes {
  fn default() -> Self {
    Self {
      user_agent: None,
      visible: true,
      transparent: false,
      url: None,
      html: None,
      initialization_scripts: vec![],
      custom_protocols: vec![],
      ipc_handler: None,
      file_drop_handler: None,
      navigation_handler: None,
      clipboard: false,
      devtools: false,
      zoom_hotkeys_enabled: false,
    }
  }
}

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to constrcut WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] privides ability to setup initialization before web engine starts.
pub struct WebViewBuilder<'a> {
  pub webview: WebViewAttributes,
  web_context: Option<&'a mut WebContext>,
  window: Window,
}

impl<'a> WebViewBuilder<'a> {
  /// Create [`WebViewBuilder`] from provided [`Window`].
  pub fn new(window: Window) -> Result<Self> {
    let webview = WebViewAttributes::default();
    let web_context = None;

    Ok(Self {
      webview,
      web_context,
      window,
    })
  }

  /// Sets whether the WebView should be transparent. Not supported on Windows 7.
  pub fn with_transparent(mut self, transparent: bool) -> Self {
    self.webview.transparent = transparent;
    self
  }

  /// Sets whether the WebView should be transparent.
  pub fn with_visible(mut self, visible: bool) -> Self {
    self.webview.visible = visible;
    self
  }

  /// Initialize javascript code when loading new pages. When webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  pub fn with_initialization_script(mut self, js: &str) -> Self {
    self.webview.initialization_scripts.push(js.to_string());
    self
  }

  /// Register custom file loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes a url string slice, and returns a two item tuple of a
  /// vector of bytes which is the content and a mimetype string of the content.
  ///
  /// # Warning
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS: `<scheme_name>://<path>` (so it will be `wry://examples` in `custom_protocol` example)
  /// - Linux: Though it's same as macOS, there's a [bug] that Origin header in the request will be
  /// empty. So the only way to pass the server is setting `Access-Control-Allow-Origin: *`.
  /// - Windows: `https://<scheme_name>.<path>` (so it will be `https://wry.examples` in `custom_protocol` example)
  /// - Android: Custom protocol on Android is fixed to `https://tauri.wry/` due to its design and
  /// our approach to use it. On Android, We only handle the scheme name and ignore the closure. So
  /// when you load the url like `wry://assets/index.html`, it will become
  /// `https://tauri.wry/assets/index.html`. Android has `assets` and `resource` path finder to
  /// locate your files in those directories. For more information, see [Loading in-app content](https://developer.android.com/guide/webapps/load-local-content) page.
  /// - iOS: Same as macOS. To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  ///
  /// [bug]: https://bugs.webkit.org/show_bug.cgi?id=229034
  #[cfg(feature = "protocol")]
  pub fn with_custom_protocol<F>(mut self, name: String, handler: F) -> Self
  where
    F: Fn(&HttpRequest) -> Result<HttpResponse> + 'static,
  {
    self
      .webview
      .custom_protocols
      .push((name, Box::new(handler)));
    self
  }

  /// Set the IPC handler to receive the message from Javascript on webview to host Rust code.
  /// The message sent from webview should call `window.ipc.postMessage("insert_message_here");`.
  pub fn with_ipc_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, String) + 'static,
  {
    self.webview.ipc_handler = Some(Box::new(handler));
    self
  }

  /// Set a handler closure to process incoming [`FileDropEvent`] of the webview.
  ///
  /// # Blocking OS Default Behavior
  /// Return `true` in the callback to block the OS' default behavior of handling a file drop.
  ///
  /// Note, that if you do block this behavior, it won't be possible to drop files on `<input type="file">` forms.
  /// Also note, that it's not possible to manually set the value of a `<input type="file">` via JavaScript for security reasons.
  #[cfg(feature = "file-drop")]
  pub fn with_file_drop_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, FileDropEvent) -> bool + 'static,
  {
    self.webview.file_drop_handler = Some(Box::new(handler));
    self
  }

  /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. The provided URL must be valid.
  pub fn with_url(mut self, url: &str) -> Result<Self> {
    self.webview.url = Some(Url::parse(url)?);
    Ok(self)
  }

  /// Load the provided HTML string when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. This will be ignored if `url` is already provided.
  ///
  /// # Warning
  /// The Page loaded from html string will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS: `http://localhost`
  /// - Linux: `http://localhost`
  /// - Windows: `null`
  /// - Android: Not supported
  /// - iOS: Not supported
  pub fn with_html(mut self, html: impl Into<String>) -> Result<Self> {
    self.webview.html = Some(html.into());
    Ok(self)
  }

  /// Set the web context that can share with multiple [`WebView`]s.
  pub fn with_web_context(mut self, web_context: &'a mut WebContext) -> Self {
    self.web_context = Some(web_context);
    self
  }

  /// Set a custom [user-agent](https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/User-Agent) for the WebView.
  pub fn with_user_agent(mut self, user_agent: &str) -> Self {
    self.webview.user_agent = Some(user_agent.to_string());
    self
  }

  /// Enable web inspector which is usually called dev tool.
  ///
  /// Note this only enables dev tool to the webview. To open it, you can call
  /// [`WebView::open_devtools`], or right click the page and open it from the context menu.
  ///
  /// ## Platform-specific
  ///
  /// - macOS: This will call private functions on **macOS**. It's still enabled if set in **debug** build on mac,
  /// but requires `devtools` feature flag to actually enable it in **release** build.
  /// - Android: Open `chrome://inspect/#devices` in Chrome to get the devtools window. Wry's `WebView` devtools API isn't supported on Android.
  /// - iOS: Open Safari > Develop > [Your Device Name] > [Your WebView] to get the devtools window.
  pub fn with_devtools(mut self, devtools: bool) -> Self {
    self.webview.devtools = devtools;
    self
  }

  /// Whether page zooming by hotkeys or gestures is enabled
  ///
  /// ## Platform-specific
  ///
  /// **macOS / Linux / Android / iOS**: Unsupported
  pub fn with_hotkeys_zoom(mut self, zoom: bool) -> Self {
    self.webview.zoom_hotkeys_enabled = zoom;
    self
  }

  /// Set a navigation handler to decide if incoming url is allowed to navigate.
  ///
  /// The closure takes a `String` parameter as url and return `bool` to determine the url. True is
  /// allowed to nivagate and false is not.
  pub fn with_navigation_handler(mut self, callback: impl Fn(String) -> bool + 'static) -> Self {
    self.webview.navigation_handler = Some(Box::new(callback));
    self
  }

  /// Consume the builder and create the [`WebView`].
  ///
  /// Platform-specific behavior:
  ///
  /// - **Unix:** This method must be called in a gtk thread. Usually this means it should be
  /// called in the same thread with the [`EventLoop`] you create.
  ///
  /// [`EventLoop`]: crate::application::event_loop::EventLoop
  pub fn build(self) -> Result<WebView> {
    let window = Rc::new(self.window);
    let webview = InnerWebView::new(window.clone(), self.webview, self.web_context)?;
    Ok(WebView { window, webview })
  }
}

/// The fundamental type to present a [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to constrcut WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebView`] presents the actuall WebView window and let you still able to perform actions
/// during event handling to it. [`WebView`] also contains the associate [`Window`] with it.
pub struct WebView {
  window: Rc<Window>,
  webview: InnerWebView,
}

// Signal the Window to drop on Linux and Windows. On mac, we need to handle several unsafe code
// blocks and raw pointer properly.
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
impl Drop for WebView {
  fn drop(&mut self) {
    unsafe {
      use crate::application::platform::unix::WindowExtUnix;
      use gtk::prelude::WidgetExtManual;
      self.window().gtk_window().destroy();
    }
  }
}

#[cfg(target_os = "windows")]
impl Drop for WebView {
  fn drop(&mut self) {
    unsafe {
      DestroyWindow(HWND(self.window.hwnd() as _));
    }
  }
}

impl WebView {
  /// Create a [`WebView`] from provided [`Window`]. Note that calling this directly loses
  /// abilities to initialize scripts, add ipc handler, and many more before starting WebView. To
  /// benefit from above features, create a [`WebViewBuilder`] instead.
  ///
  /// Platform-specific behavior:
  ///
  /// - **Unix:** This method must be called in a gtk thread. Usually this means it should be
  /// called in the same thread with the [`EventLoop`] you create.
  ///
  /// [`EventLoop`]: crate::application::event_loop::EventLoop
  pub fn new(window: Window) -> Result<Self> {
    WebViewBuilder::new(window)?.build()
  }

  /// Get the [`Window`] associate with the [`WebView`]. This can let you perform window related
  /// actions.
  pub fn window(&self) -> &Window {
    &self.window
  }

  /// Evaluate and run javascript code. Must be called on the same thread who created the
  /// [`WebView`]. Use [`EventLoopProxy`] and a custom event to send scripts from other threads.
  ///
  /// [`EventLoopProxy`]: crate::application::event_loop::EventLoopProxy
  pub fn evaluate_script(&self, js: &str) -> Result<()> {
    self.webview.eval(js)
  }

  /// Launch print modal for the webview content.
  pub fn print(&self) -> Result<()> {
    self.webview.print();
    Ok(())
  }

  /// Resize the WebView manually. This is only required on Windows because its WebView API doesn't
  /// provide a way to resize automatically.
  pub fn resize(&self) -> Result<()> {
    #[cfg(target_os = "windows")]
    self.webview.resize(HWND(self.window.hwnd() as _))?;
    Ok(())
  }

  /// Moves Focus to the Webview control.
  ///
  /// It's usually safe to call `focus` method on `Window` which would also focus to `WebView` except Windows.
  /// Focussing to `Window` doesn't mean focussing to `WebView` on Windows. For example, if you have
  /// an input field on webview and lost focus, you will have to explicitly click the field even you
  /// re-focus the window. And if you focus to `WebView`, it will lost focus to the `Window`.
  pub fn focus(&self) {
    self.webview.focus();
  }

  /// Open the web inspector which is usually called dev tool.
  ///
  /// ## Platform-specific
  ///
  /// - **Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {
    self.webview.open_devtools();
  }

  /// Close the web inspector which is usually called dev tool.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows / Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {
    self.webview.close_devtools();
  }

  /// Gets the devtool window's current vibility state.
  ///
  /// ## Platform-specific
  ///
  /// - **Windows / Android / iOS:** Not supported.
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool {
    self.webview.is_devtools_open()
  }

  pub fn inner_size(&self) -> PhysicalSize<u32> {
    #[cfg(target_os = "macos")]
    {
      let scale_factor = self.window.scale_factor();
      self.webview.inner_size(scale_factor)
    }
    #[cfg(not(target_os = "macos"))]
    self.window.inner_size()
  }

  /// Set the webview zoom level
  ///
  /// ## Platform-specific:
  ///
  /// - **Android**: Not supported.
  /// - **macOS**: available on macOS 11+ only.
  /// - **iOS**: available on iOS 14+ only.
  pub fn zoom(&self, scale_factor: f64) {
    self.webview.zoom(scale_factor);
  }

  #[cfg(target_os = "android")]
  pub fn run(self, env: JNIEnv, jclass: JClass, jobject: JObject) -> jobject {
    self.webview.run(env, jclass, jobject).unwrap()
  }

  #[cfg(target_os = "android")]
  pub fn ipc_handler(window: &Window, arg: String) {
    InnerWebView::ipc_handler(window, arg)
  }
}

/// An event enumeration sent to [`FileDropHandler`].
#[non_exhaustive]
#[derive(Debug, Serialize, Clone)]
pub enum FileDropEvent {
  /// The file(s) have been dragged onto the window, but have not been dropped yet.
  Hovered(Vec<PathBuf>),
  /// The file(s) have been dropped onto the window.
  Dropped(Vec<PathBuf>),
  /// The file drop was aborted.
  Cancelled,
}

/// Get Webview/Webkit version on current platform.
pub fn webview_version() -> Result<String> {
  platform_webview_version()
}

/// Additional methods on `WebView` that are specific to Windows.
#[cfg(target_os = "windows")]
pub trait WebviewExtWindows {
  /// Returns WebView2 Controller
  fn controller(&self) -> ICoreWebView2Controller;
}

#[cfg(target_os = "windows")]
impl WebviewExtWindows for WebView {
  fn controller(&self) -> ICoreWebView2Controller {
    self.webview.controller.clone()
  }
}

/// Additional methods on `WebView` that are specific to Linux.
#[cfg(target_os = "linux")]
pub trait WebviewExtUnix {
  /// Returns Webkit2gtk Webview handle
  fn webview(&self) -> Rc<webkit2gtk::WebView>;
}

#[cfg(target_os = "linux")]
impl WebviewExtUnix for WebView {
  fn webview(&self) -> Rc<webkit2gtk::WebView> {
    self.webview.webview.clone()
  }
}

/// Additional methods on `WebView` that are specific to macOS.
#[cfg(target_os = "macos")]
pub trait WebviewExtMacOS {
  /// Returns WKWebView handle
  fn webview(&self) -> cocoa::base::id;
  /// Returns WKWebView manager [(userContentController)](https://developer.apple.com/documentation/webkit/wkscriptmessagehandler/1396222-usercontentcontroller) handle
  fn manager(&self) -> cocoa::base::id;
  /// Returns NSWindow associated with the WKWebView webview
  fn ns_window(&self) -> cocoa::base::id;
}

#[cfg(target_os = "macos")]
impl WebviewExtMacOS for WebView {
  fn webview(&self) -> cocoa::base::id {
    self.webview.webview.clone()
  }

  fn manager(&self) -> cocoa::base::id {
    self.webview.manager.clone()
  }

  fn ns_window(&self) -> cocoa::base::id {
    self.webview.ns_window.clone()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn should_get_webview_version() {
    if let Err(error) = webview_version() {
      panic!("{}", error);
    }
  }
}

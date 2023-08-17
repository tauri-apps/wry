// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! [`WebView`] struct and associated types.

mod proxy;
mod web_context;

pub use web_context::WebContext;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "android")]
pub mod prelude {
  pub use super::android::{binding::*, dispatch, find_class, setup, Context};
}
#[cfg(target_os = "android")]
pub use android::JniHandle;
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
use crate::{application::dpi::PhysicalPosition, Result};
#[cfg(target_os = "windows")]
use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
#[cfg(target_os = "windows")]
use windows::{Win32::Foundation::HWND, Win32::UI::WindowsAndMessaging::DestroyWindow};

use std::{borrow::Cow, path::PathBuf, rc::Rc};

pub use proxy::{ProxyConfig, ProxyEndpoint};
pub use url::Url;

#[cfg(target_os = "windows")]
use crate::application::platform::windows::WindowExtWindows;
use crate::application::{dpi::PhysicalSize, window::Window};

use http::{Request, Response};

pub struct WebViewAttributes {
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
  pub url: Option<Url>,
  /// Headers used when loading the requested `url`.
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
  /// Initialize javascript code when loading new pages. When webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  ///
  /// ## Platform-specific
  ///
  /// - **Android:** The Android WebView does not provide an API for initialization scripts,
  /// so we prepend them to each HTML head. They are only implemented on custom protocol URLs.
  pub initialization_scripts: Vec<String>,
  /// Register custom file loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes a [Request] and returns a [Response].
  ///
  /// # Warning
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS and Linux: `<scheme_name>://<path>` (so it will be `wry://examples` in `custom_protocol` example). On Linux, You need to enable `linux-headers` feature flag.
  /// - Windows: `https://<scheme_name>.<path>` (so it will be `https://wry.examples` in `custom_protocol` example)
  /// - Android: Custom protocol on Android is fixed to `https://tauri.wry/` due to its design and
  /// our approach to use it. On Android, We only handle the scheme name and ignore the closure. So
  /// when you load the url like `wry://assets/index.html`, it will become
  /// `https://tauri.wry/assets/index.html`. Android has `assets` and `resource` path finder to
  /// locate your files in those directories. For more information, see [Loading in-app content](https://developer.android.com/guide/webapps/load-local-content) page.
  /// - iOS: Same as macOS. To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  ///
  /// [bug]: https://bugs.webkit.org/show_bug.cgi?id=229034
  pub custom_protocols: Vec<(
    String,
    Box<dyn Fn(&Request<Vec<u8>>) -> Result<Response<Cow<'static, [u8]>>>>,
  )>,
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
  /// allow to navigate and false is not.
  pub navigation_handler: Option<Box<dyn Fn(String) -> bool>>,

  /// Set a download started handler to manage incoming downloads.
  ///
  /// The closure takes two parameters - the first is a `String` representing the url being downloaded from and and the
  /// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
  /// parameter can be used to set the download location by assigning a new path to it - the assigned path _must_ be
  /// absolute. The closure returns a `bool` to allow or deny the download.
  pub download_started_handler: Option<Box<dyn FnMut(String, &mut PathBuf) -> bool>>,

  /// Sets a download completion handler to manage downloads that have finished.
  ///
  /// The closure is fired when the download completes, whether it was successful or not.
  /// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
  /// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
  /// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
  /// did not succeed, and may instead indicate some other failure - always check the third parameter if you need to
  /// know if the download succeeded.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS**: The second parameter indicating the path the file was saved to is always empty, due to API
  /// limitations.
  pub download_completed_handler: Option<Rc<dyn Fn(String, Option<PathBuf>, bool) + 'static>>,

  /// Set a new window handler to decide if incoming url is allowed to open in a new window.
  ///
  /// The closure take a `String` parameter as url and return `bool` to determine the url. True is
  /// allow to navigate and false is not.
  pub new_window_req_handler: Option<Box<dyn Fn(String) -> bool>>,

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
  /// - **Android / iOS:** Unsupported.
  pub back_forward_navigation_gestures: bool,

  /// Set a handler closure to process the change of the webview's document title.
  pub document_title_changed_handler: Option<Box<dyn Fn(&Window, String)>>,

  /// Run the WebView with incognito mode. Note that WebContext will be ingored if incognito is
  /// enabled.
  ///
  /// ## Platform-specific:
  ///
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
}

impl Default for WebViewAttributes {
  fn default() -> Self {
    Self {
      user_agent: None,
      visible: true,
      transparent: false,
      background_color: None,
      url: None,
      headers: None,
      html: None,
      initialization_scripts: vec![],
      custom_protocols: vec![],
      ipc_handler: None,
      file_drop_handler: None,
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
    }
  }
}

#[cfg(windows)]
#[derive(Clone)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  additional_browser_args: Option<String>,
  browser_accelerator_keys: bool,
  theme: Option<Theme>,
  https_scheme: bool,
}
#[cfg(windows)]
impl Default for PlatformSpecificWebViewAttributes {
  fn default() -> Self {
    Self {
      additional_browser_args: None,
      browser_accelerator_keys: true, // This is WebView2's default behavior
      theme: None,
      https_scheme: false, // To match macOS & Linux behavior in the context of mixed content.
    }
  }
}
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
  target_os = "macos",
  target_os = "ios",
))]
#[derive(Default)]
pub(crate) struct PlatformSpecificWebViewAttributes;

#[cfg(target_os = "android")]
#[derive(Default)]
pub(crate) struct PlatformSpecificWebViewAttributes {
  on_webview_created: Option<
    Box<
      dyn Fn(
          prelude::Context,
        ) -> std::result::Result<(), tao::platform::android::ndk_glue::jni::errors::Error>
        + Send,
    >,
  >,
  with_asset_loader: bool,
  asset_loader_domain: Option<String>,
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

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] provides ability to setup initialization before web engine starts.
pub struct WebViewBuilder<'a> {
  pub webview: WebViewAttributes,
  platform_specific: PlatformSpecificWebViewAttributes,
  web_context: Option<&'a mut WebContext>,
  window: Window,
}

impl<'a> WebViewBuilder<'a> {
  /// Create [`WebViewBuilder`] from provided [`Window`].
  pub fn new(window: Window) -> Result<Self> {
    let webview = WebViewAttributes::default();
    let web_context = None;
    #[allow(clippy::default_constructed_unit_structs)]
    let platform_specific = PlatformSpecificWebViewAttributes::default();

    Ok(Self {
      webview,
      web_context,
      window,
      platform_specific,
    })
  }

  /// Indicates whether horizontal swipe gestures trigger backward and forward page navigation.
  ///
  /// ## Platform-specific:
  ///
  /// - **Android / iOS:** Unsupported.
  pub fn with_back_forward_navigation_gestures(mut self, gesture: bool) -> Self {
    self.webview.back_forward_navigation_gestures = gesture;
    self
  }

  /// Sets whether the WebView should be transparent.
  ///
  /// ## Platform-specific:
  ///
  /// **Windows 7**: Not supported.
  pub fn with_transparent(mut self, transparent: bool) -> Self {
    self.webview.transparent = transparent;
    self
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
  pub fn with_background_color(mut self, background_color: RGBA) -> Self {
    self.webview.background_color = Some(background_color);
    self
  }

  /// Sets whether the WebView should be transparent.
  pub fn with_visible(mut self, visible: bool) -> Self {
    self.webview.visible = visible;
    self
  }

  /// Sets whether all media can be played without user interaction.
  pub fn with_autoplay(mut self, autoplay: bool) -> Self {
    self.webview.autoplay = autoplay;
    self
  }

  /// Initialize javascript code when loading new pages. When webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  ///
  /// ## Platform-specific
  ///
  /// - **Android:** The Android WebView does not provide an API for initialization scripts,
  /// so we prepend them to each HTML head. They are only implemented on custom protocol URLs.
  pub fn with_initialization_script(mut self, js: &str) -> Self {
    if !js.is_empty() {
      self.webview.initialization_scripts.push(js.to_string());
    }
    self
  }

  /// Register custom file loading protocols with pairs of scheme uri string and a handling
  /// closure.
  ///
  /// The closure takes a [Request] and returns a [Response]
  ///
  /// # Warning
  /// Pages loaded from custom protocol will have different Origin on different platforms. And
  /// servers which enforce CORS will need to add exact same Origin header in `Access-Control-Allow-Origin`
  /// if you wish to send requests with native `fetch` and `XmlHttpRequest` APIs. Here are the
  /// different Origin headers across platforms:
  ///
  /// - macOS and Linux: `<scheme_name>://<path>` (so it will be `wry://examples` in `custom_protocol` example). On Linux, You need to enable `linux-headers` feature flag.
  /// - Windows: `https://<scheme_name>.<path>` (so it will be `https://wry.examples` in `custom_protocol` example)
  /// - Android: For loading content from the `assets` folder (which is copied to the Andorid apk) please
  /// use the function [`with_asset_loader`] from [`WebViewBuilderExtAndroid`] instead.
  /// This function on Android can only be used to serve assets you can embed in the binary or are
  /// elsewhere in Android (provided the app has appropriate access), but not from the `assets`
  /// folder which lives within the apk. For the cases where this can be used, it works the same as in macOS and Linux.
  /// - iOS: Same as macOS. To get the path of your assets, you can call [`CFBundle::resources_path`](https://docs.rs/core-foundation/latest/core_foundation/bundle/struct.CFBundle.html#method.resources_path). So url like `wry://assets/index.html` could get the html file in assets directory.
  ///
  /// [bug]: https://bugs.webkit.org/show_bug.cgi?id=229034
  #[cfg(feature = "protocol")]
  pub fn with_custom_protocol<F>(mut self, name: String, handler: F) -> Self
  where
    F: Fn(&Request<Vec<u8>>) -> Result<Response<Cow<'static, [u8]>>> + 'static,
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

  /// Load the provided URL with given headers when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. The provided URL must be valid.
  pub fn with_url_and_headers(mut self, url: &str, headers: http::HeaderMap) -> Result<Self> {
    self.webview.url = Some(url.parse()?);
    self.webview.headers = Some(headers);
    Ok(self)
  }

  /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. The provided URL must be valid.
  pub fn with_url(mut self, url: &str) -> Result<Self> {
    self.webview.url = Some(Url::parse(url)?);
    self.webview.headers = None;
    Ok(self)
  }

  /// Load the provided HTML string when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. This will be ignored if `url` is provided.
  ///
  /// # Warning
  ///
  /// The Page loaded from html string will have `null` origin.
  ///
  /// ## PLatform-specific:
  ///
  /// - **Windows:** the string can not be larger than 2 MB (2 * 1024 * 1024 bytes) in total size
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

  /// Enable or disable web inspector which is usually called dev tool.
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
  /// allowed to navigate and false is not.
  pub fn with_navigation_handler(mut self, callback: impl Fn(String) -> bool + 'static) -> Self {
    self.webview.navigation_handler = Some(Box::new(callback));
    self
  }

  /// Set a download started handler to manage incoming downloads.
  ///
  /// The closure takes two parameters - the first is a `String` representing the url being downloaded from and and the
  /// second is a mutable `PathBuf` reference that (possibly) represents where the file will be downloaded to. The latter
  /// parameter can be used to set the download location by assigning a new path to it - the assigned path _must_ be
  /// absolute. The closure returns a `bool` to allow or deny the download.
  pub fn with_download_started_handler(
    mut self,
    started_handler: impl FnMut(String, &mut PathBuf) -> bool + 'static,
  ) -> Self {
    self.webview.download_started_handler = Some(Box::new(started_handler));
    self
  }

  /// Sets a download completion handler to manage downloads that have finished.
  ///
  /// The closure is fired when the download completes, whether it was successful or not.
  /// The closure takes a `String` representing the URL of the original download request, an `Option<PathBuf>`
  /// potentially representing the filesystem path the file was downloaded to, and a `bool` indicating if the download
  /// succeeded. A value of `None` being passed instead of a `PathBuf` does not necessarily indicate that the download
  /// did not succeed, and may instead indicate some other failure - always check the third parameter if you need to
  /// know if the download succeeded.
  ///
  /// ## Platform-specific:
  ///
  /// - **macOS**: The second parameter indicating the path the file was saved to is always empty, due to API
  /// limitations.
  pub fn with_download_completed_handler(
    mut self,
    download_completed_handler: impl Fn(String, Option<PathBuf>, bool) + 'static,
  ) -> Self {
    self.webview.download_completed_handler = Some(Rc::new(download_completed_handler));
    self
  }

  /// Enables clipboard access for the page rendered on **Linux** and **Windows**.
  ///
  /// macOS doesn't provide such method and is always enabled by default. But you still need to add menu
  /// item accelerators to use shortcuts.
  pub fn with_clipboard(mut self, clipboard: bool) -> Self {
    self.webview.clipboard = clipboard;
    self
  }

  /// Set a new window request handler to decide if incoming url is allowed to be opened.
  ///
  /// The closure takes a `String` parameter as url and return `bool` to determine if the url can be
  /// opened in a new window. Returning true will open the url in a new window, whilst returning false
  /// will neither open a new window nor allow any navigation.
  pub fn with_new_window_req_handler(
    mut self,
    callback: impl Fn(String) -> bool + 'static,
  ) -> Self {
    self.webview.new_window_req_handler = Some(Box::new(callback));
    self
  }

  /// Sets whether clicking an inactive window also clicks through to the webview. Default is `false`.
  ///
  /// ## Platform-specific
  ///
  /// This configuration only impacts macOS.
  pub fn with_accept_first_mouse(mut self, accept_first_mouse: bool) -> Self {
    self.webview.accept_first_mouse = accept_first_mouse;
    self
  }

  /// Set a handler closure to process the change of the webview's document title.
  pub fn with_document_title_changed_handler(
    mut self,
    callback: impl Fn(&Window, String) + 'static,
  ) -> Self {
    self.webview.document_title_changed_handler = Some(Box::new(callback));
    self
  }

  /// Run the WebView with incognito mode. Note that WebContext will be ingored if incognito is
  /// enabled.
  ///
  /// ## Platform-specific:
  ///
  /// - **Android:** Unsupported yet.
  pub fn with_incognito(mut self, incognito: bool) -> Self {
    self.webview.incognito = incognito;
    self
  }

  /// Set a handler to process page loading events.
  ///
  /// The handler will be called when the webview begins the indicated loading event.
  pub fn with_on_page_load_handler(
    mut self,
    handler: impl Fn(PageLoadEvent, String) + 'static,
  ) -> Self {
    self.webview.on_page_load_handler = Some(Box::new(handler));
    self
  }

  /// Set a proxy configuration for the webview.
  ///
  /// - **macOS**: Requires macOS 14.0+ and the `mac-proxy` feature flag to be enabled. Supports HTTP CONNECT and SOCKSv5 proxies.
  /// - **Windows / Linux**: Supports HTTP CONNECT and SOCKSv5 proxies.
  /// - **Android / iOS:** Not supported.
  pub fn with_proxy_config(mut self, configuration: ProxyConfig) -> Self {
    self.webview.proxy_config = Some(configuration);
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
    let webview = InnerWebView::new(
      window.clone(),
      self.webview,
      self.platform_specific,
      self.web_context,
    )?;
    Ok(WebView { window, webview })
  }
}

#[cfg(windows)]
pub trait WebViewBuilderExtWindows {
  /// Pass additional args to Webview2 upon creating the webview.
  ///
  /// ## Warning
  ///
  /// By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
  /// `--autoplay-policy=no-user-gesture-required` if autoplay is enabled
  /// and `--proxy-server=<scheme>://<host>:<port>` if a proxy is set.
  /// so if you use this method, you have to add these arguments yourself if you want to keep the same behavior.
  fn with_additional_browser_args<S: Into<String>>(self, additional_args: S) -> Self;

  /// Determines whether browser-specific accelerator keys are enabled. When this setting is set to
  /// `false`, it disables all accelerator keys that access features specific to a web browser.
  /// The default value is `true`. See the following link to know more details.
  ///
  /// https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings#arebrowseracceleratorkeysenabled
  fn with_browser_accelerator_keys(self, enabled: bool) -> Self;

  /// Specifies the theme of webview2. This affects things like `prefers-color-scheme`.
  ///
  /// Defaults to [`Theme::Auto`] which will follow the OS defaults.
  fn with_theme(self, theme: Theme) -> Self;

  /// Determines whether the custom protocols should use `https://<scheme>.localhost` instead of the default `http://<scheme>.localhost`.
  ///
  /// Using a `http` scheme will allow mixed content when trying to fetch `http` endpoints
  /// and is therefore less secure but will match the behavior of the `<scheme>://localhost` protocols used on macOS and Linux.
  ///
  /// The default value is `false`.
  fn with_https_scheme(self, enabled: bool) -> Self;
}

#[cfg(windows)]
impl WebViewBuilderExtWindows for WebViewBuilder<'_> {
  fn with_additional_browser_args<S: Into<String>>(mut self, additional_args: S) -> Self {
    self.platform_specific.additional_browser_args = Some(additional_args.into());
    self
  }

  fn with_browser_accelerator_keys(mut self, enabled: bool) -> Self {
    self.platform_specific.browser_accelerator_keys = enabled;
    self
  }

  fn with_theme(mut self, theme: Theme) -> Self {
    self.platform_specific.theme = Some(theme);
    self
  }

  fn with_https_scheme(mut self, enabled: bool) -> Self {
    self.platform_specific.https_scheme = enabled;
    self
  }
}

#[cfg(target_os = "android")]
pub trait WebViewBuilderExtAndroid {
  fn on_webview_created<
    F: Fn(
        prelude::Context<'_, '_>,
      ) -> std::result::Result<(), tao::platform::android::ndk_glue::jni::errors::Error>
      + Send
      + 'static,
  >(
    self,
    f: F,
  ) -> Self;

  /// Use [WebviewAssetLoader](https://developer.android.com/reference/kotlin/androidx/webkit/WebViewAssetLoader)
  /// to load assets from Android's `asset` folder when using `with_url` as `<protocol>://assets/` (e.g.:
  /// `wry://assets/index.html`). Note that this registers a custom protocol with the provided
  /// String, similar to [`with_custom_protocol`], but also sets the WebViewAssetLoader with the
  /// necessary domain (which is fixed as `<protocol>.assets`). This cannot be used in conjunction
  /// to `with_custom_protocol` for Android, as it changes the way in which requests are handled.
  #[cfg(feature = "protocol")]
  fn with_asset_loader(self, protocol: String) -> Self;
}

#[cfg(target_os = "android")]
impl WebViewBuilderExtAndroid for WebViewBuilder<'_> {
  fn on_webview_created<
    F: Fn(
        prelude::Context<'_, '_>,
      ) -> std::result::Result<(), tao::platform::android::ndk_glue::jni::errors::Error>
      + Send
      + 'static,
  >(
    mut self,
    f: F,
  ) -> Self {
    self.platform_specific.on_webview_created = Some(Box::new(f));
    self
  }

  #[cfg(feature = "protocol")]
  fn with_asset_loader(mut self, protocol: String) -> Self {
    // register custom protocol with empty Response return,
    // this is necessary due to the need of fixing a domain
    // in WebViewAssetLoader.
    self.webview.custom_protocols.push((
      protocol.clone(),
      Box::new(|_| Ok(Response::builder().body(Vec::new().into())?)),
    ));
    self.platform_specific.with_asset_loader = true;
    self.platform_specific.asset_loader_domain = Some(format!("{}.assets", protocol));
    self
  }
}

/// The fundamental type to present a [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to construct WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebView`] presents the actual WebView window and let you still able to perform actions
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

  /// Get the current url of the webview
  pub fn url(&self) -> Url {
    self.webview.url()
  }

  /// Evaluate and run javascript code. Must be called on the same thread who created the
  /// [`WebView`]. Use [`EventLoopProxy`] and a custom event to send scripts from other threads.
  ///
  /// [`EventLoopProxy`]: crate::application::event_loop::EventLoopProxy
  ///
  pub fn evaluate_script(&self, js: &str) -> Result<()> {
    self
      .webview
      .eval(js, None::<Box<dyn Fn(String) + Send + 'static>>)
  }

  /// Evaluate and run javascript code with callback function. The evaluation result will be
  /// serialized into a JSON string and passed to the callback function. Must be called on the
  /// same thread who created the [`WebView`]. Use [`EventLoopProxy`] and a custom event to
  /// send scripts from other threads.
  ///
  /// [`EventLoopProxy`]: crate::application::event_loop::EventLoopProxy
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
    self.webview.print();
    Ok(())
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

  /// Gets the devtool window's current visibility state.
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
  pub fn load_url(&self, url: &str) {
    self.webview.load_url(url)
  }

  /// Navigate to the specified url using the specified headers
  pub fn load_url_with_headers(&self, url: &str, headers: http::HeaderMap) {
    self.webview.load_url_with_headers(url, headers)
  }

  /// Clear all browsing data
  pub fn clear_all_browsing_data(&self) -> Result<()> {
    self.webview.clear_all_browsing_data()
  }
}

/// An event enumeration sent to [`FileDropHandler`].
#[non_exhaustive]
#[derive(Debug, Serialize, Clone)]
pub enum FileDropEvent {
  /// The file(s) have been dragged onto the window, but have not been dropped yet.
  Hovered {
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: PhysicalPosition<f64>,
  },
  /// The file(s) have been dropped onto the window.
  Dropped {
    paths: Vec<PathBuf>,
    /// The position of the mouse cursor.
    position: PhysicalPosition<f64>,
  },
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

  // Changes the webview2 theme.
  fn set_theme(&self, theme: Theme);
}

#[cfg(target_os = "windows")]
impl WebviewExtWindows for WebView {
  fn controller(&self) -> ICoreWebView2Controller {
    self.webview.controller.clone()
  }

  fn set_theme(&self, theme: Theme) {
    self.webview.set_theme(theme)
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
    self.webview.webview
  }

  fn manager(&self) -> cocoa::base::id {
    self.webview.manager
  }

  fn ns_window(&self) -> cocoa::base::id {
    self.webview.ns_window
  }
}

/// Additional methods on `WebView` that are specific to iOS.
#[cfg(target_os = "ios")]
pub trait WebviewExtIOS {
  /// Returns WKWebView handle
  fn webview(&self) -> cocoa::base::id;
  /// Returns WKWebView manager [(userContentController)](https://developer.apple.com/documentation/webkit/wkscriptmessagehandler/1396222-usercontentcontroller) handle
  fn manager(&self) -> cocoa::base::id;
}

#[cfg(target_os = "ios")]
impl WebviewExtIOS for WebView {
  fn webview(&self) -> cocoa::base::id {
    self.webview.webview
  }

  fn manager(&self) -> cocoa::base::id {
    self.webview.manager
  }
}

#[cfg(target_os = "android")]
/// Additional methods on `WebView` that are specific to Android
pub trait WebviewExtAndroid {
  fn handle(&self) -> JniHandle;
}

#[cfg(target_os = "android")]
impl WebviewExtAndroid for WebView {
  fn handle(&self) -> JniHandle {
    JniHandle
  }
}

#[derive(Debug, Clone, Copy)]
pub enum Theme {
  Dark,
  Light,
  Auto,
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

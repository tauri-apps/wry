// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use crate::{Result, RpcRequest, RpcResponse, WindowProxy};

use std::{
  fs::read,
  path::{Path, PathBuf},
};

/// The RPC handler to Communicate between the host Rust code and Javascript on webview.
///
/// The communication is done via [JSON-RPC](https://www.jsonrpc.org).
/// Users can pass a `WindowRpcHandler` to [`Application::add_window_with_configs`](crate::Application::add_window_with_configs) to register an incoming
/// request handler and reply with responses that are passed back to Javascript. On the Javascript
/// side the client is exposed via `window.rpc` with two public methods:
///
/// 1. The `call()` function accepts a method name and parameters and expects a reply.
/// 2. The `notify()` function accepts a method name and parameters but does not expect a reply.
///
/// Both functions return promises but `notify()` resolves immediately.
///
/// # Example
///
/// ```no_run
/// use wry::{Application, Result, WindowProxy, RpcRequest, RpcResponse};
///
/// fn main() -> Result<()> {
///     let mut app = Application::new()?;
///     let handler = Box::new(|proxy: WindowProxy, mut req: RpcRequest| {
///       // Handle the request of type `RpcRequest` and reply with `Option<RpcResponse>`,
///       // use the `req.method` field to determine which action to take.
///       //
///       // If the handler returns a `RpcResponse` it is immediately evaluated
///       // in the calling webview.
///       //
///       // Use the `WindowProxy` to modify the window, eg: `set_fullscreen` etc.
///       //
///       // If the request is a notification (no `id` field) then the handler
///       // can just return `None`.
///       //
///       // If an `id` field is present and the handler wants to execute asynchronous
///       // code it can return `None` but then *must* later evaluate either
///       // `RpcResponse::into_result_script()` or `RpcResponse::into_error_script()`
///       // in the webview to ensure the promise is resolved or rejected and removed
///       // from the cache.
///       None
///     });
///     app.add_window_with_configs(Default::default(), Some(handler), vec![], None)?;
///     app.run();
///     Ok(())
/// }
/// ```
///
/// Then in Javascript use `call()` to call a remote method and get a response:
///
/// ```javascript
/// async function callRemoteMethod() {
///   let result = await window.rpc.call('remoteMethod', param1, param2);
///   // Do something with the result
/// }
/// ```
///
/// If Javascript code wants to use a callback style it is easy to alias a function to a method call:
///
/// ```javascript
/// function someRemoteMethod() {
///   return window.rpc.call(arguments.callee.name, Array.prototype.slice(arguments, 0));
/// }
/// ```
pub type WindowRpcHandler = Box<dyn Fn(WindowProxy, RpcRequest) -> Option<RpcResponse> + Send>;

/// A protocol to define custom URL scheme for handling tasks like loading assets.
pub struct CustomProtocol {
  /// The name of the custom URL scheme.
  pub name: String,
  /// The closure that takes the URL as parameter and returns the contents bytes that
  /// WebView going to load.
  pub handler: Box<dyn Fn(&str) -> Result<Vec<u8>> + Send>,
}

/// An icon used for the window title bar, taskbar, etc.
#[derive(Debug, Clone)]
pub struct Icon(pub(crate) Vec<u8>);

impl Icon {
  /// Creates an icon from the file.
  pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
    Ok(Self(read(path)?))
  }
  /// Creates an icon from raw bytes.
  pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
    Ok(Self(bytes.into()))
  }
}

/// Attributes to use when creating a webview window.
#[derive(Debug, Clone)]
pub struct Attributes {
  /// Whether the window is resizable or not.
  ///
  /// The default is `true`.
  pub resizable: bool,

  /// The title of the window in the title bar.
  ///
  /// The default is `"wry"`.
  pub title: String,

  /// Whether the window should be maximized upon creation.
  ///
  /// The default is `false`.
  pub maximized: bool,

  /// Whether the window should be immediately visible upon creation.
  ///
  /// The default is `true`.
  pub visible: bool,

  /// Whether the WebView window should be transparent. If this is true, writing colors
  /// with alpha values different than `1.0` will produce a transparent window.
  ///
  /// The default is `false`.
  pub transparent: bool,

  /// Whether the window should have borders and bars.
  ///
  /// The default is `true`.
  pub decorations: bool,

  /// Whether the window should always be on top of other windows.
  ///
  /// The default is `false`.
  pub always_on_top: bool,

  /// The width of the window.
  ///
  /// The default is `800.0`.
  pub width: f64,

  /// The height of the window.
  ///
  /// The default is `600.0`.
  pub height: f64,

  /// The minimum width of the window.
  ///
  /// The default is `None`.
  pub min_width: Option<f64>,

  /// The minimum height of the window.
  ///
  /// The default is `None`.
  pub min_height: Option<f64>,

  /// The maximum width of the window.
  ///
  /// The default is `None`.
  pub max_width: Option<f64>,

  /// The maximum height of the window.
  ///
  /// The default is `None`.
  pub max_height: Option<f64>,

  /// The horizontal position of the window's top left corner.
  ///
  /// The default is `None`.
  pub x: Option<f64>,

  /// The vertical position of the window's top left corner.
  ///
  /// The default is `None`.
  pub y: Option<f64>,

  /// Whether to start the window in fullscreen or not.
  ///
  /// The default is `false`.
  pub fullscreen: bool,

  /// The window icon.
  ///
  /// The default is `None`.
  pub icon: Option<Icon>,

  /// Whether to hide the window icon in the taskbar/dock.
  ///
  /// The default is `false`
  pub skip_taskbar: bool,

  /// The URL to be loaded in the webview window.
  ///
  /// The default is `None`.
  pub url: Option<String>,

  /// Javascript Code to be initialized when loading new pages.
  ///
  /// The default is an empty vector.
  pub initialization_scripts: Vec<String>,

  /// Webview user data path.
  ///
  /// The default is `None`.
  pub user_data_path: Option<PathBuf>,
}

impl Attributes {
  pub(crate) fn split(self) -> (InnerWindowAttributes, InnerWebViewAttributes) {
    (
      InnerWindowAttributes {
        resizable: self.resizable,
        title: self.title,
        maximized: self.maximized,
        visible: self.visible,
        transparent: self.transparent,
        decorations: self.decorations,
        always_on_top: self.always_on_top,
        width: self.width,
        height: self.height,
        min_width: self.min_width,
        min_height: self.min_height,
        max_width: self.max_width,
        max_height: self.max_height,
        x: self.x,
        y: self.y,
        fullscreen: self.fullscreen,
        icon: self.icon,
        skip_taskbar: self.skip_taskbar,
      },
      InnerWebViewAttributes {
        transparent: self.transparent,
        url: self.url,
        initialization_scripts: self.initialization_scripts,
        user_data_path: self.user_data_path,
      },
    )
  }
}

impl Default for Attributes {
  #[inline]
  fn default() -> Self {
    Self {
      resizable: true,
      title: "wry".to_owned(),
      maximized: false,
      visible: true,
      transparent: false,
      decorations: true,
      always_on_top: false,
      width: 800.0,
      height: 600.0,
      min_width: None,
      min_height: None,
      max_width: None,
      max_height: None,
      x: None,
      y: None,
      fullscreen: false,
      icon: None,
      skip_taskbar: false,
      url: None,
      initialization_scripts: vec![],
      user_data_path: None,
    }
  }
}

pub(crate) struct InnerWindowAttributes {
  pub resizable: bool,
  pub title: String,
  pub maximized: bool,
  pub visible: bool,
  pub transparent: bool,
  pub decorations: bool,
  pub always_on_top: bool,
  pub width: f64,
  pub height: f64,
  pub min_width: Option<f64>,
  pub min_height: Option<f64>,
  pub max_width: Option<f64>,
  pub max_height: Option<f64>,
  pub x: Option<f64>,
  pub y: Option<f64>,
  pub fullscreen: bool,
  pub icon: Option<Icon>,
  pub skip_taskbar: bool,
}

pub(crate) struct InnerWebViewAttributes {
  pub transparent: bool,
  pub url: Option<String>,
  pub initialization_scripts: Vec<String>,
  pub user_data_path: Option<PathBuf>,
}

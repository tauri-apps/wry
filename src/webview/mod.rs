// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! [`WebView`] struct and associated types.

mod mimetype;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::*;
#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
mod winrt;
#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
use winrt::*;
#[cfg(target_os = "windows")]
#[cfg(feature = "win32")]
mod win32;
#[cfg(target_os = "windows")]
#[cfg(feature = "win32")]
use win32::*;

use crate::{Error, Result};

use std::{
  path::PathBuf,
  rc::Rc,
  sync::mpsc::{channel, Receiver, Sender},
};

use serde_json::Value;
use url::Url;

#[cfg(target_os = "windows")]
use crate::application::platform::windows::WindowExtWindows;
use crate::application::window::Window;
#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
use windows_webview2::Windows::Win32::WindowsAndMessaging::HWND;

// Helper so all platforms handle RPC messages consistently.
fn rpc_proxy(
  window: &Window,
  js: String,
  handler: &dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>,
) -> Result<Option<String>> {
  let req = serde_json::from_str::<RpcRequest>(&js)
    .map_err(|e| Error::RpcScriptError(e.to_string(), js))?;

  let mut response = (handler)(window, req);
  // Got a synchronous response so convert it to a script to be evaluated
  if let Some(mut response) = response.take() {
    if let Some(id) = response.id {
      let js = if let Some(error) = response.error.take() {
        RpcResponse::get_error_script(id, error)?
      } else if let Some(result) = response.result.take() {
        RpcResponse::get_result_script(id, result)?
      } else {
        // No error or result, assume a positive response
        // with empty result (ACK)
        RpcResponse::get_result_script(id, Value::Null)?
      };
      Ok(Some(js))
    } else {
      Ok(None)
    }
  } else {
    Ok(None)
  }
}

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to constrcut WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] privides ability to setup initialization before web engine starts.
pub struct WebViewBuilder {
  transparent: bool,
  tx: Sender<String>,
  rx: Receiver<String>,
  initialization_scripts: Vec<String>,
  window: Window,
  url: Option<Url>,
  custom_protocols: Vec<(String, Box<dyn Fn(&Window, &str) -> Result<Vec<u8>>>)>,
  rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
  file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
  data_directory: Option<PathBuf>,
}

impl WebViewBuilder {
  /// Create [`WebViewBuilder`] from provided [`Window`].
  pub fn new(window: Window) -> Result<Self> {
    let (tx, rx) = channel();

    Ok(Self {
      tx,
      rx,
      initialization_scripts: vec![],
      window,
      url: None,
      transparent: false,
      custom_protocols: vec![],
      rpc_handler: None,
      file_drop_handler: None,
      data_directory: None,
    })
  }

  /// Whether the WebView window should be transparent. If this is true, writing colors
  /// with alpha values different than `1.0` will produce a transparent window.
  pub fn with_transparent(mut self, transparent: bool) -> Self {
    self.transparent = transparent;
    self
  }

  /// Initialize javascript code when loading new pages. Everytime webview load a new page, this
  /// initialization code will be executed. It is guaranteed that code is executed before
  /// `window.onload`.
  pub fn with_initialization_script(mut self, js: &str) -> Self {
    self.initialization_scripts.push(js.to_string());
    self
  }

  /// Whether the WebView window should have a custom user data path. This is usefull in Windows
  /// when a bundled application can't have the webview data inside `Program Files`.
  pub fn with_data_directory(mut self, data_directory: PathBuf) -> Self {
    self.data_directory.replace(data_directory);
    self
  }

  /// Create a [`Dispatcher`] to send evaluation scripts to the WebView. [`WebView`] is not thread
  /// safe because it must be run on the main thread who creates it. [`Dispatcher`] can let you
  /// send the scripts from other threads.
  pub fn dispatcher(&self) -> Dispatcher {
    Dispatcher(self.tx.clone())
  }

  /// Register custom file loading protocol
  #[cfg(feature = "protocol")]
  pub fn with_custom_protocol<F>(mut self, name: String, handler: F) -> Self
  where
    F: Fn(&Window, &str) -> Result<Vec<u8>> + 'static,
  {
    self.custom_protocols.push((name, Box::new(handler)));
    self
  }

  /// Set the RPC handler to Communicate between the host Rust code and Javascript on webview.
  ///
  /// The communication is done via [JSON-RPC](https://www.jsonrpc.org). Users can use this to register an incoming
  /// request handler and reply with responses that are passed back to Javascript. On the Javascript
  /// side the client is exposed via `window.rpc` with two public methods:
  ///
  /// 1. The `call()` function accepts a method name and parameters and expects a reply.
  /// 2. The `notify()` function accepts a method name and parameters but does not expect a reply.
  ///
  /// Both functions return promises but `notify()` resolves immediately.
  pub fn with_rpc_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, RpcRequest) -> Option<RpcResponse> + 'static,
  {
    let js = r#"
            (function() {
                function Rpc() {
                    const self = this;
                    this._promises = {};

                    // Private internal function called on error
                    this._error = (id, error) => {
                        if(this._promises[id]){
                            this._promises[id].reject(error);
                            delete this._promises[id];
                        }
                    }

                    // Private internal function called on result
                    this._result = (id, result) => {
                        if(this._promises[id]){
                            this._promises[id].resolve(result);
                            delete this._promises[id];
                        }
                    }

                    // Call remote method and expect a reply from the handler
                    this.call = function(method) {
                        let array = new Uint32Array(1);
                        window.crypto.getRandomValues(array);
                        const id = array[0];
                        const params = Array.prototype.slice.call(arguments, 1);
                        const payload = {jsonrpc: "2.0", id, method, params};
                        const promise = new Promise((resolve, reject) => {
                            self._promises[id] = {resolve, reject};
                        });
                        window.external.invoke(JSON.stringify(payload));
                        return promise;
                    }

                    // Send a notification without an `id` so no reply is expected.
                    this.notify = function(method) {
                        const params = Array.prototype.slice.call(arguments, 1);
                        const payload = {jsonrpc: "2.0", method, params};
                        window.external.invoke(JSON.stringify(payload));
                        return Promise.resolve();
                    }
                }
                window.external = window.external || {};
                window.external.rpc = new Rpc();
                window.rpc = window.external.rpc;
            })();
            "#;

    self.initialization_scripts.push(js.to_string());
    self.rpc_handler = Some(Box::new(handler));
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
    self.file_drop_handler = Some(Box::new(handler));
    self
  }

  /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the
  /// [`WebView`]. The provided URL must be valid.
  pub fn with_url(mut self, url: &str) -> Result<Self> {
    self.url = Some(Url::parse(url)?);
    Ok(self)
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
      self.initialization_scripts,
      self.url,
      self.transparent,
      self.custom_protocols,
      self.rpc_handler,
      self.file_drop_handler,
      self.data_directory,
    )?;
    Ok(WebView {
      window,
      webview,
      tx: self.tx,
      rx: self.rx,
    })
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
  tx: Sender<String>,
  rx: Receiver<String>,
}

// Signal the Window to drop on Linux and Windows. On mac, we need to handle several unsafe code
// blocks and raw pointer properly.
impl Drop for WebView {
  fn drop(&mut self) {
    #[cfg(target_os = "linux")]
    {
      use crate::application::platform::unix::WindowExtUnix;
      use gtk::GtkWindowExt;
      self.window.gtk_window().close();
    }
    #[cfg(target_os = "windows")]
    unsafe {
      use winapi::{shared::windef::HWND, um::winuser::DestroyWindow};
      DestroyWindow(self.window.hwnd() as HWND);
    }
  }
}

impl WebView {
  /// Create a [`WebView`] from provided [`Window`]. Note that calling this directly loses
  /// abilities to initialize scripts, add rpc handler, and many more before starting WebView. To
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

  /// Dispatch javascript code to be evaluated later. Note this will not actually run the
  /// scripts being dispatched. Users need to call [`WebView::evaluate_script`] to execute them.
  pub fn dispatch_script(&mut self, js: &str) -> Result<()> {
    self.tx.send(js.to_string())?;
    Ok(())
  }

  /// Create a [`Dispatcher`] to send evaluation scripts to the WebView. [`WebView`] is not thread
  /// safe because it must be run on the main thread who creates it. [`Dispatcher`] can let you
  /// send the scripts from other threads.
  pub fn dispatcher(&self) -> Dispatcher {
    Dispatcher(self.tx.clone())
  }

  /// Get the [`Window`] associate with the [`WebView`]. This can let you perform window related
  /// actions.
  pub fn window(&self) -> &Window {
    &self.window
  }

  /// Evaluate the scripts sent from [`Dispatcher`]s.
  pub fn evaluate_script(&self) -> Result<()> {
    while let Ok(js) = self.rx.try_recv() {
      self.webview.eval(&js)?;
    }

    Ok(())
  }

  /// Launch print modal for the webview content.
  /// Works only on macOS for now.
  pub fn print(&self) -> Result<()> {
    self.webview.print();
    Ok(())
  }

  /// Resize the WebView manually. This is required on Windows because its WebView API doesn't
  /// provide a way to resize automatically.
  pub fn resize(&self) -> Result<()> {
    #[cfg(target_os = "windows")]
    #[cfg(feature = "winrt")]
    self.webview.resize(HWND(self.window.hwnd() as _))?;
    #[cfg(target_os = "windows")]
    #[cfg(feature = "win32")]
    self.webview.resize(self.window.hwnd())?;
    Ok(())
  }
}

#[derive(Clone)]
/// A channel sender to dispatch javascript code to for the [`WebView`] to evaluate it.
///
/// [`WebView`] is not thread safe because it must be run on main thread who creates it.
/// [`Dispatcher`] can let you send scripts from other thread.
pub struct Dispatcher(Sender<String>);

impl Dispatcher {
  /// Dispatch javascript code to be evaluated later. Note this will not actually run the
  /// scripts being dispatched. Users need to call [`WebView::evaluate_script`] to execute them.
  pub fn dispatch_script(&self, js: &str) -> Result<()> {
    self.0.send(js.to_string())?;
    Ok(())
  }
}

const RPC_VERSION: &str = "2.0";

/// RPC request message.
///
/// This usually passes to the [`RpcHandler`] or [`WindowRpcHandler`](crate::WindowRpcHandler) as
/// the parameter. You don't create this by yourself.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
  jsonrpc: String,
  pub id: Option<Value>,
  pub method: String,
  pub params: Option<Value>,
}

/// RPC response message which being sent back to the Javascript side.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
  jsonrpc: String,
  pub(crate) id: Option<Value>,
  pub(crate) result: Option<Value>,
  pub(crate) error: Option<Value>,
}

impl RpcResponse {
  /// Create a new result response.
  pub fn new_result(id: Option<Value>, result: Option<Value>) -> Self {
    Self {
      jsonrpc: RPC_VERSION.to_string(),
      id,
      result,
      error: None,
    }
  }

  /// Create a new error response.
  pub fn new_error(id: Option<Value>, error: Option<Value>) -> Self {
    Self {
      jsonrpc: RPC_VERSION.to_string(),
      id,
      error,
      result: None,
    }
  }

  /// Get a script that resolves the promise with a result.
  pub fn get_result_script(id: Value, result: Value) -> Result<String> {
    let retval = serde_json::to_string(&result)?;
    Ok(format!(
      "window.external.rpc._result({}, {})",
      id.to_string(),
      retval
    ))
  }

  /// Get a script that rejects the promise with an error.
  pub fn get_error_script(id: Value, result: Value) -> Result<String> {
    let retval = serde_json::to_string(&result)?;
    Ok(format!(
      "window.external.rpc._error({}, {})",
      id.to_string(),
      retval
    ))
  }
}

/// An event enumeration sent to [`FileDropHandler`].
#[derive(Debug, Serialize, Clone)]
pub enum FileDropEvent {
  /// The file(s) have been dragged onto the window, but have not been dropped yet.
  Hovered(Vec<PathBuf>),
  /// The file(s) have been dropped onto the window.
  Dropped(Vec<PathBuf>),
  /// The file drop was aborted.
  Cancelled,
}

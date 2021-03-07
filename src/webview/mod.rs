//! [`WebView`] struct and associated types.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::*;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::*;
#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
use win::*;

#[cfg(feature = "file-drop")]
use crate::file_drop::FileDropHandler;

use crate::{Error, Result};

use std::sync::mpsc::{channel, Receiver, Sender};

use serde_json::Value;
use url::Url;

#[cfg(target_os = "linux")]
use gtk::ApplicationWindow as Window;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

pub type RpcHandler = Box<dyn Fn(RpcRequest) -> Option<RpcResponse> + Send>;

// Helper so all platforms handle RPC messages consistently.
fn rpc_proxy(js: String, handler: &RpcHandler) -> Result<Option<String>> {
    let req = serde_json::from_str::<RpcRequest>(&js)
        .map_err(|e| Error::RpcScriptError(e.to_string(), js))?;

    let mut response = (handler)(req);
    // Got a synchronous response so convert it to a script to be evaluated
    if let Some(mut response) = response.take() {
        if let Some(id) = response.id {
            let js = if let Some(error) = response.error.take() {
                RpcResponse::into_error_script(id, error)?
            } else if let Some(result) = response.result.take() {
                RpcResponse::into_result_script(id, result)?
            } else {
                // No error or result, assume a positive response
                // with empty result (ACK)
                RpcResponse::into_result_script(id, Value::Null)?
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
    custom_protocol: Option<(String, Box<dyn Fn(&str) -> Result<Vec<u8>>>)>,
    rpc_handler: Option<RpcHandler>,

    #[cfg(feature = "file-drop")]
    file_drop_handler: Option<FileDropHandler>,
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
            custom_protocol: None,
            rpc_handler: None,

            #[cfg(feature = "file-drop")]
            file_drop_handler: None,
        })
    }

    /// Whether the WebView window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.transparent = transparent;
        self
    }

    /// Initialize javascript code when loading new pages. Everytime webview load a new page, this
    /// initialization code will be executed. It is guaranteed that code is executed before
    /// `window.onload`.
    pub fn initialize_script(mut self, js: &str) -> Self {
        self.initialization_scripts.push(js.to_string());
        self
    }

    /// Create a [`Dispatcher`] to send evaluation scripts to the WebView. [`WebView`] is not thread
    /// safe because it must be run on the main thread who creates it. [`Dispatcher`] can let you
    /// send the scripts from other threads.
    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.tx.clone())
    }

    /// Register custom file loading protocol
    pub fn register_protocol<F>(mut self, name: String, handler: F) -> Self
    where
        F: Fn(&str) -> Result<Vec<u8>> + 'static,
    {
        self.custom_protocol = Some((name, Box::new(handler)));
        self
    }

    /// Set the RPC handler.
    pub fn set_rpc_handler(mut self, handler: RpcHandler) -> Self {
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
                        const id = Math.floor(Math.random() * Number.MAX_SAFE_INTEGER);
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
        self.rpc_handler = Some(handler);
        self
    }

    #[cfg(feature = "file-drop")]
    pub fn set_file_drop_handler(mut self, handler: Option<FileDropHandler>) -> Self {
        self.file_drop_handler = handler;
        self
    }

    /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the
    /// [`WebView`]. The provided URL must be valid.
    pub fn load_url(mut self, url: &str) -> Result<Self> {
        self.url = Some(Url::parse(url)?);
        Ok(self)
    }

    /// Consume the builder and create the [`WebView`].
    pub fn build(self) -> Result<WebView> {
        let webview = InnerWebView::new(
            &self.window,
            self.initialization_scripts,
            self.url,
            self.transparent,
            self.custom_protocol,
            self.rpc_handler,
            #[cfg(feature = "file-drop")]
            self.file_drop_handler,
        )?;
        Ok(WebView {
            window: self.window,
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
    window: Window,
    webview: InnerWebView,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl WebView {
    /// Create a [`WebView`] from provided [`Window`]. Note that calling this directly loses
    /// abilities to initialize scripts, add callbacks, and many more before starting WebView. To
    /// benefit from above features, create a [`WebViewBuilder`] instead.
    pub fn new(window: Window) -> Result<Self> {
        Self::new_with_configs(window, false)
    }

    /// Create a [`WebView`] from provided [`Window`] along with several configurations.
    /// Note that calling this directly loses abilities to initialize scripts, add callbacks, and
    /// many more before starting WebView. To benefit from above features, create a
    /// [`WebViewBuilder`] instead.
    pub fn new_with_configs(window: Window, transparent: bool) -> Result<Self> {
        let picky_none: Option<(String, Box<dyn Fn(&str) -> Result<Vec<u8>>>)> = None;

        let webview = InnerWebView::new(
            &window,
            vec![],
            None,
            transparent,
            picky_none,
            None,
            #[cfg(feature = "file-drop")]
            None,
        )?;

        let (tx, rx) = channel();

        Ok(Self {
            window,
            webview,
            tx,
            rx,
        })
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

    /// Resize the WebView manually. This is required on Windows because its WebView API doesn't
    /// provide a way to resize automatically.
    pub fn resize(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
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

pub(crate) trait WV: Sized {
    type Window;

    fn new<F: 'static + Fn(&str) -> Result<Vec<u8>>>(
        window: &Self::Window,
        scripts: Vec<String>,
        url: Option<Url>,
        transparent: bool,
        custom_protocol: Option<(String, F)>,
        rpc_handler: Option<RpcHandler>,

        #[cfg(feature = "file-drop")] file_drop_handler: Option<FileDropHandler>,
    ) -> Result<Self>;

    fn eval(&self, js: &str) -> Result<()>;
}

const RPC_VERSION: &str = "2.0";

/// RPC request message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// RPC response message.
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
    pub fn into_result_script(id: Value, result: Value) -> Result<String> {
        let retval = serde_json::to_string(&result)?;
        Ok(format!(
            "window.external.rpc._result({}, {})",
            id.to_string(),
            retval
        ))
    }

    /// Get a script that rejects the promise with an error.
    pub fn into_error_script(id: Value, result: Value) -> Result<String> {
        let retval = serde_json::to_string(&result)?;
        Ok(format!(
            "window.external.rpc._error({}, {})",
            id.to_string(),
            retval
        ))
    }
}

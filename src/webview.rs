//! [`WebView`] struct and associated types.

use crate::platform::{InnerWebView, RpcRequest, RpcResponse, CALLBACKS, RPC_CALLBACK_NAME};
use crate::application::WindowProxy;
use crate::Result;

use std::sync::{Arc, mpsc::{channel, Receiver, Sender}};
#[cfg(not(target_os = "linux"))]
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use serde_json::Value;
use url::Url;

#[cfg(target_os = "linux")]
use gtk::{ApplicationWindow as Window, ApplicationWindowExt};
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

//pub type RpcProxyHandler = Box<dyn Fn(&Dispatcher, RpcRequest) -> Option<RpcResponse> + Send + Sync>;
pub type RpcHandler = Box<dyn Fn(&WindowProxy, RpcRequest) -> Option<RpcResponse> + Send + Sync>;

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
    window_id: i64,
    custom_protocol: Option<(String, Box<dyn Fn(&str) -> Result<Vec<u8>>>)>,
    rpc_handler: Option<(
        WindowProxy,
        Arc<RpcHandler>,
    )>,
}

impl WebViewBuilder {
    /// Create [`WebViewBuilder`] from provided [`Window`].
    pub fn new(window: Window) -> Result<Self> {
        let (tx, rx) = channel();
        #[cfg(not(target_os = "linux"))]
        let window_id = {
            let mut hasher = DefaultHasher::new();
            window.id().hash(&mut hasher);
            hasher.finish() as i64
        };
        #[cfg(target_os = "linux")]
        let window_id = window.get_id() as i64;

        Ok(Self {
            tx,
            rx,
            initialization_scripts: vec![],
            window,
            url: None,
            transparent: false,
            window_id,
            custom_protocol: None,
            rpc_handler: None,
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

    /// Add a callback function to the WebView. The callback takse a dispatcher, a sequence number,
    /// and a vector of arguments passed from callers as parameters.
    ///
    /// It uses RPC to communicate with javascript side and the sequence number is used to record
    /// how many times has this callback been called. Arguments passed from callers is a vector of
    /// serde values for you to decide how to handle them. IF you need to evaluate any code on
    /// javascript side, you can use the dispatcher to send them.
    pub fn add_callback<F>(mut self, name: &str, f: F) -> Self
    where
        F: FnMut(&Dispatcher, i32, Vec<Value>) -> Result<()> + Send + 'static,
    {

        let js = format!(
            r#"
            (function() {{
                var name = {:?};
                var RPC = window._rpc = (window._rpc || {{nextSeq: 1}});
                window[name] = function() {{
                var seq = RPC.nextSeq++;
                var promise = new Promise(function(resolve, reject) {{
                    RPC[seq] = {{
                    resolve: resolve,
                    reject: reject,
                    }};
                }});
                window.external.invoke(JSON.stringify({{
                    callback: {:?},
                    payload: {{
                        jsonrpc: '2.0',
                        id: seq,
                        method: name,
                        params: Array.prototype.slice.call(arguments),
                    }}
                }}));
                return promise;
                }}
            }})()
            "#,
            name,
            name,
        );
        self.initialization_scripts.push(js);

        let window_id = self.window_id;
        CALLBACKS.lock().unwrap().insert(
            (window_id, name.to_string()),
            (Box::new(f), Dispatcher(self.tx.clone())),
        );
        self
    }

    /// Set the RPC handler.
    pub(crate) fn set_rpc_handler(mut self, proxy: WindowProxy, handler: Arc<RpcHandler>) -> Self {
        let js =
            r#"
            function Rpc() {
                this._callback = '__rpc__';
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
                    const msg = {callback: this._callback, payload};
                    const promise = new Promise((resolve, reject) => {
                        this._promises[id] = {resolve, reject};
                    });
                    window.external.invoke(JSON.stringify(msg));
                    return promise;
                }

                // Send a notification without an `id` so no reply is expected.
                this.notify = function(method) {
                    const params = Array.prototype.slice.call(arguments, 1);
                    const payload = {jsonrpc: "2.0", method, params};
                    const msg = {callback: this._callback, payload};
                    window.external.invoke(JSON.stringify(msg));
                    return Promise.resolve();
                }
            }
            window.rpc = window.external.rpc = new Rpc();
            "#;
        self.initialization_scripts.push(js.to_string());
        self.rpc_handler = Some((proxy, handler));
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
        let webview = InnerWebView::new(&window, vec![], None, transparent, picky_none, None)?;
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
        rpc_handler: Option<(
            WindowProxy,
            Arc<RpcHandler>,
        )>,
    ) -> Result<Self>;

    fn eval(&self, js: &str) -> Result<()>;
}

use crate::platform::{InnerWebView, CALLBACKS};
use crate::Result;

use std::sync::mpsc::{channel, Receiver, Sender};

use url::Url;

#[cfg(target_os = "linux")]
use gtk::ApplicationWindow as Window;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

const DEBUG: bool = true;

/// Builder type of [`WebView`].
///
/// [`WebViewBuilder`] / [`WebView`] are the basic building blocks to constrcut WebView contents and
/// scripts for those who prefer to control fine grained window creation and event handling.
/// [`WebViewBuilder`] privides ability to setup initialization before web engine starts.
pub struct WebViewBuilder {
    inner: WebView,
    url: Option<Url>,
}

impl WebViewBuilder {
    /// Create [`WebViewBuilder`] from provided [`Window`].
    pub fn new(window: Window) -> Result<Self> {
        Ok(Self {
            inner: WebView::new(window)?,
            url: None,
        })
    }

    /// Initialize javascript code when loading new pages. Everytime webview load a new page, this
    /// initialization code will be executed. It is guaranteed that code is executed before
    /// `window.onload`.
    pub fn initialize_script(self, js: &str) -> Result<Self> {
        self.inner.webview.init(js)?;
        Ok(self)
    }

    /// Create a [`Dispatcher`] to send evaluation scripts to the WebView. [`WebView`] is not thread
    /// safe because it must be run on the main thread who creates it. [`Dispatcher`] can let you
    /// send the scripts from other threads.
    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.inner.tx.clone())
    }

    /// Add a callback function to the WebView. The callback takse a dispatcher, a sequence number,
    /// and a vector of arguments passed from callers as parameters.
    ///
    /// It uses RPC to communicate with javascript side and the sequence number is used to record
    /// how many times has this callback been called. Arguments passed from callers is a vector of
    /// serde values for you to decide how to handle them. IF you need to evaluate any code on
    /// javascript side, you can use the dispatcher to send them.
    pub fn add_callback<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send + 'static,
    {
        let js = format!(
            r#"var name = {:?};
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
                    id: seq,
                    method: name,
                    params: Array.prototype.slice.call(arguments),
                }}));
                return promise;
                }}
            "#,
            name
        );
        self.inner.webview.init(&js)?;
        let dispatcher = self.dispatcher();
        CALLBACKS
            .lock()
            .unwrap()
            .insert(name.to_string(), (Box::new(f), dispatcher));
        Ok(self)
    }

    /// Load the provided URL when the builder calling [`WebViewBuilder::build`] to create the
    /// [`WebView`]. The provided URL must be valid.
    pub fn load_url(mut self, url: &str) -> Result<Self> {
        self.url = Some(Url::parse(url)?);
        Ok(self)
    }

    /// Consume the builder and create the [`WebView`].
    pub fn build(self) -> Result<WebView> {
        if let Some(url) = self.url {
            if url.cannot_be_a_base() {
                self.inner.webview.navigate_to_string(url.as_str())?;
            } else {
                self.inner.webview.navigate(url.as_str())?;
            }
        }
        Ok(self.inner)
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
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(&window, DEBUG)?;
        #[cfg(target_os = "linux")]
        let webview = InnerWebView::new(&window, DEBUG);
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
    pub fn resize(&self) {
        #[cfg(target_os = "windows")]
        self.webview.resize(self.window.hwnd());
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

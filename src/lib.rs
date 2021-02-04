#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod platform;

use crate::platform::{InnerWebView, CALLBACKS};

use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};

#[cfg(target_os = "linux")]
use gtk::Window;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

const DEBUG: bool = true;

pub struct WebViewBuilder {
    inner: WebView,
    url: Option<String>,
}

impl WebViewBuilder {
    pub fn new(window: Window) -> Result<Self> {
        Ok(Self {
            inner: WebView::new(window)?,
            url: None,
        })
    }

    pub fn init(self, js: &str) -> Result<Self> {
        self.inner.webview.init(js)?;
        Ok(self)
    }

    pub fn dispatch_sender(&self) -> DispatchSender {
        DispatchSender(self.inner.tx.clone())
    }

    // TODO implement bind here
    pub fn bind<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Send + 'static,
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
        CALLBACKS
            .lock()
            .unwrap()
            .insert(name.to_string(), Box::new(f));
        Ok(self)
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = Some(url.to_string());
        self
    }

    pub fn build(self) -> Result<WebView> {
        if let Some(url) = self.url {
            self.inner.webview.navigate(&url)?;
        }
        Ok(self.inner)
    }
}

thread_local!(static EVAL: RefCell<Option<Receiver<String>>> = RefCell::new(None));

pub struct WebView {
    window: Window,
    webview: InnerWebView,
    tx: Sender<String>,
}

impl WebView {
    pub fn new(window: Window) -> Result<Self> {
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(&window, DEBUG)?;
        #[cfg(target_os = "linux")]
        let webview = InnerWebView::new(&window, DEBUG);
        let (tx, rx) = channel();
        EVAL.with(|e| {
            *e.borrow_mut() = Some(rx);
        });
        Ok(Self {
            window,
            webview,
            tx,
        })
    }

    pub fn dispatch(&mut self, js: &str) -> Result<()> {
        self.tx.send(js.to_string())?;
        Ok(())
    }

    pub fn dispatch_sender(&self) -> DispatchSender {
        DispatchSender(self.tx.clone())
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn evaluate(&mut self) -> Result<()> {
        EVAL.with(|e| -> Result<()> {
            let e = &*e.borrow();
            if let Some(rx) = e {
                while let Ok(js) = rx.try_recv() {
                    self.webview.eval(&js)?;
                }
            } else {
                return Err(Error::EvalError);
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn resize(&self) {
        #[cfg(target_os = "windows")]
        self.webview.resize(self.window.hwnd());
    }
}

pub struct DispatchSender(Sender<String>);

impl DispatchSender {
    pub fn send(&self, js: &str) -> Result<()> {
        self.0.send(js.to_string())?;
        Ok(())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to initialize the script")]
    InitScriptError,
    #[error("Script is not evaluated on the same thread with its webview!")]
    EvalError,
    #[error(transparent)]
    NulError(#[from] std::ffi::NulError),
    #[cfg(not(target_os = "linux"))]
    #[error(transparent)]
    OsError(#[from] winit::error::OsError),
    #[error(transparent)]
    SenderError(#[from] SendError<String>),
    #[cfg(target_os = "windows")]
    #[error("Windows error: {0:?}")]
    WinrtError(windows::Error),
}

#[cfg(target_os = "windows")]
impl From<windows::Error> for Error {
    fn from(error: windows::Error) -> Self {
        Error::WinrtError(error)
    }
}

#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub use winit::*;

mod platform;

use crate::platform::*;

use std::fmt;
use std::sync::mpsc::{channel, Receiver, SendError, Sender};

//use thiserror::Error;

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowExtMacOS;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
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

    pub fn eval_sender(&self) -> EvalSender {
        EvalSender(self.inner.tx.clone())
    }

    pub fn bind<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Send + 'static,
    {
        self.inner.webview.bind(name, f)?;
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

pub struct WebView {
    window: Window,
    webview: InnerWebView,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl WebView {
    pub fn new(window: Window) -> Result<Self> {
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(window.ns_view(), DEBUG)?;
        let (tx, rx) = channel();
        Ok(Self {
            window,
            webview,
            tx,
            rx,
        })
    }

    pub fn eval(&mut self, js: &str) -> Result<()> {
        self.tx.send(js.to_string())?;
        Ok(())
    }

    pub fn eval_sender(&self) -> EvalSender {
        EvalSender(self.tx.clone())
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn dispatch(&mut self) -> Result<()> {
        while let Ok(js) = self.rx.try_recv() {
            self.webview.eval(&js)?;
        }

        Ok(())
    }

    // TODO resize
}

pub struct EvalSender(Sender<String>);

impl EvalSender {
    pub fn send(&self, js: &str) -> Result<()> {
        self.0.send(js.to_string())?;
        Ok(())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    NulError(#[from] std::ffi::NulError),
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

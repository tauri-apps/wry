#[macro_use]
extern crate serde;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub use winit::*;

mod platform;

use crate::platform::*;

use std::fmt;

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

    pub fn eval_handler(&self) -> EvalHandler {
        EvalHandler(self.inner.webview.clone())
    }

    pub fn bind<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Sync + Send + 'static,
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
}

impl WebView {
    pub fn new(window: Window) -> Result<Self> {
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(window.ns_view(), DEBUG)?;
        Ok(Self { window, webview })
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        self.webview.eval(js)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    // TODO resize
}

pub struct EvalHandler(InnerWebView);

impl EvalHandler {
    pub fn eval(&self, js: &str) -> Result<()> {
        self.0.eval(js)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InitError,
    NulError(std::ffi::NulError),
    #[cfg(target_os = "windows")]
    WinrtError(windows::Error),
    OsError(winit::error::OsError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InitError => "Fail to initialize instance".fmt(f),
            Error::NulError(e) => e.fmt(f),
            #[cfg(target_os = "windows")]
            Error::WinrtError(e) => format!("{:?}", e).fmt(f),
            Error::OsError(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(target_os = "windows")]
impl From<windows::Error> for Error {
    fn from(error: windows::Error) -> Self {
        Error::WinrtError(error)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(error: std::ffi::NulError) -> Self {
        Error::NulError(error)
    }
}

impl From<winit::error::OsError> for Error {
    fn from(error: winit::error::OsError) -> Self {
        Error::OsError(error)
    }
}

#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub mod application;
pub mod platform;
pub mod webview;

pub use application::{Application, Callback, WebViewAttributes};
pub use webview::{Dispatcher, WebView, WebViewBuilder};

use std::sync::mpsc::SendError;

use url::ParseError;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[cfg(target_os = "linux")]
    #[error(transparent)]
    GlibError(#[from] glib::Error),
    #[cfg(target_os = "linux")]
    #[error(transparent)]
    GlibBoolError(#[from] glib::BoolError),
    #[error("Failed to initialize the script")]
    InitScriptError,
    #[error(transparent)]
    NulError(#[from] std::ffi::NulError),
    #[cfg(not(target_os = "linux"))]
    #[error(transparent)]
    OsError(#[from] winit::error::OsError),
    #[error(transparent)]
    SenderError(#[from] SendError<String>),
    #[error(transparent)]
    UrlError(#[from] ParseError),
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

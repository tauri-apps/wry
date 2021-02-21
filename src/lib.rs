//! Wry is a Cross-platform WebView rendering library.
//!
//! There are two main ways to build WebView windows: [`Application`] and build by yourself.
//!
//! # Building WebView windows through [`Application`]
//!
//! [`Application`] is the recommended way to build the WebView windows. It provides ergonomic and
//! unified APIs across all platforms. To get started, you simply create an [`Application`] first:
//!
//! ```no_run
//! let application = Application::new()?;
//! ```
//!
//! Once you have your application instance, you can create the WebView window by calling
//! [`Application::add_window`]. You can provide [`Attributes`] and [`Callback`] as
//! arguments to configure the WebView window. If you don't have any preference, you could just set
//! them with `Default::default()` and `None`.
//!
//! ```no_run
//! let attributes = Attributes {
//!     url: Some("https://www.google.com".to_string()),
//!     // Initialization scripts can be used to define javascript functions and variables.
//!     initialization_scripts: vec![
//!         String::from("breads = NaN"),
//!         String::from("menacing = 'ゴ'"),
//!     ],
//!     ..Default::default()
//! };
//! // Callback defines a rust function to be called on javascript side later. Below is a function
//! // which will print the list of parameters after 8th calls.
//! let callback = Callback {
//!     name: "world".to_owned(),
//!     function: Box::new(|proxy, sequence, requests| {
//!         // Proxy is like a window handle for you to send message events to the corresponding
//!         // webview window. You can use it to adjust window and evaluate javascript code like
//!         // below. This is useful when you want to perform any action in javascript.
//!         proxy.evaluate_script("console.log(menacing);").unwrap();
//!         // Sequence is a number counting how many times this function being called.
//!         if sequence < 8 {
//!             println!("{} seconds has passed.", sequence);
//!         } else {
//!             // Requests is a vector of parameters passed from the caller.
//!             println!("{:?}", requests);
//!         }
//!         0
//!     }),
//! };
//!
//! let window = app.add_window(attributes, Some(vec![callback]))?;
//! ```
//!
//! Run the application with run in the end. This will consume the instance and run the application
//! on current thread.
//!
//! ```no_run
//! application.run();
//! ```
//!
//! # Building WebView windows by yourself
//!
//! If you want to control whole windows creation and events handling, you can use
//! [`WebViewBuilder`] / [`WebView`] under [webview] module and [platform] module to build it all
//! by yourself. [platform] module re-exports [winit] for you to build the window across all
//! platforms except Linux. We still need Gtk's library to build the WebView, so it's [gtk-rs] on
//! Linux.
//!
//! [winit]: https://crates.io/crates/winit
//! [gtk-rs]: https://crates.io/crates/gtk
//!

#[macro_use]
extern crate serde;
#[macro_use]
extern crate thiserror;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

mod application;
pub mod platform;
pub mod webview;

pub use application::{
    Application, ApplicationProxy, Attributes, Callback, Icon, Message, WindowId, WindowMessage,
    WindowProxy,
};
pub use serde_json::Value;
pub(crate) use webview::{Dispatcher, WebView, WebViewBuilder};

#[cfg(not(target_os = "linux"))]
use winit::{event_loop::EventLoopClosed, window::BadIcon};

use std::sync::mpsc::{RecvError, SendError};

use url::ParseError;

/// Convinient type alias of Result type for wry.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors returned by wry.
#[derive(Error, Debug)]
pub enum Error {
    #[cfg(not(target_os = "linux"))]
    #[error(transparent)]
    EventLoopClosed(#[from] EventLoopClosed<Message>),
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
    ReceiverError(#[from] RecvError),
    #[error(transparent)]
    SenderError(#[from] SendError<String>),
    #[error(transparent)]
    SendMessageError(#[from] SendError<Message>),
    #[error(transparent)]
    UrlError(#[from] ParseError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[cfg(not(target_os = "linux"))]
    #[error("Icon error: {0}")]
    Icon(#[from] BadIcon),
    #[cfg(target_os = "windows")]
    #[error(transparent)]
    WebView2Error(#[from] webview2::Error),
}

#[macro_use]
extern crate serde;
#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

pub use winit::*;

mod platform;

use crate::platform::*;

use std::ffi::c_void;
use std::fmt;
use std::os::raw::c_char;

#[cfg(target_os = "macos")]
use winit::platform::macos::WindowExtMacOS;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub struct WebViewBuilder {
    inner: WebView,
    url: Option<String>,
}

impl WebViewBuilder {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: WebView::new()?,
            url: None,
        })
    }

    pub fn init(self, js: &str) -> Result<Self> {
        self.inner.webview.init(js)?;
        Ok(self)
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
    events: Option<EventLoop<()>>,
    window: Window,
    webview: InnerWebView,
}

impl WebView {
    pub fn new() -> Result<Self> {
        let events = EventLoop::new();
        let window = Window::new(&events)?;
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(window.ns_view())?;
        Ok(Self {
            events: Some(events),
            window,
            webview,
        })
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        self.webview.eval(js)
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    #[cfg(target_os = "macos")]
    pub fn run(mut self) -> Result<()> {
        if let Some(events) = self.events.take() {
            events.run(move |event, _, control_flow| {
                *control_flow = ControlFlow::Wait;

                match event {
                    Event::NewEvents(StartCause::Init) => {}
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    Event::WindowEvent {
                        event: WindowEvent::Resized(_),
                        ..
                    } => {}
                    _ => (),
                }
            });
        }
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub fn run(mut self) -> Result<()> {
        if let Some(events) = self.events.take() {
            events.run(move |event, _, control_flow| {
                *control_flow = ControlFlow::Wait;

                match event {
                    Event::NewEvents(StartCause::Init) => {}
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    Event::WindowEvent {
                        event: WindowEvent::Resized(_),
                        ..
                    } => {
                        self.webview.resize(self.window.hwnd());
                    }
                    _ => (),
                }
            });
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
extern "C" {
    fn ivector(js: *const c_char) -> *mut c_void;
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

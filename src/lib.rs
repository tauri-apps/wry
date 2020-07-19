mod sys;

use crate::sys::*;

use std::fmt;

use winit::{
    event::{Event, WindowEvent, StartCause},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};
use winit::platform::windows::WindowExtWindows;

pub struct Weebview {
    events: EventLoop<()>,
    window: Window,
    webview: WebView,
}

impl Weebview {
    pub fn new() -> Self {
        let events = EventLoop::new();
        let window = Window::new(&events).unwrap();
        let webview = WebView::new(window.hwnd()).unwrap();

        Self {
            events,
            window,
            webview,
        }
    }

    pub fn navigate(&self, url: &str) {
        self.webview.navigate(url);
    }

    pub fn run(self) {
        let window = self.window;
        let webview = self.webview;
        self.events.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
    
            match event {
                Event::NewEvents(StartCause::Init) => {
                },
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    webview.resize(window.hwnd() as *mut _);
                },
                _ => (),
            }
        });
    }
}

#[derive(Debug)]
pub enum Error {
    WinrtError(winrt::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::WinrtError(e) => format!("{:?}", e).fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl From<winrt::Error> for Error {
    fn from(error: winrt::Error) -> Self {
        Error::WinrtError(error)
    }
}

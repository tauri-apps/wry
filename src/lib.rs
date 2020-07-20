mod sys;

pub use crate::sys::*;

use std::fmt;

// pub struct Weebview<'a>(WebView<'a>);

// impl<'a> Weebview<'a> {
//     pub fn new() -> Result<Self, Error> {
//         Ok(Self(WebView::new(true)?))
//     }

//     pub fn navigate(&self, url: &str) -> Result<(), Error> {
//         self.0.navigate(url)
//     }

//     pub fn init(&self, js: &str) -> Result<(), Error> {
//         self.0.init(js)
//     }

//     pub fn eval(&self, js: &str) -> Result<(), Error> {
//         self.0.eval(js)
//     }

//     pub fn bind<F>(&mut self, name: &'a str, fn_: F) -> Result<(), Error>
//         where F: Fn(usize, serde_json::Value) -> usize + 'static
//     {
//         self.0.bind(name, fn_)
//     }

//     pub fn run(self) {
//         self.0.run();
//         // let window = self.window;
//         // let webview = self.webview;
//         // self.events.run(move |event, _, control_flow| {
//         //     *control_flow = ControlFlow::Wait;

//         //     match event {
//         //         Event::NewEvents(StartCause::Init) => {}
//         //         Event::WindowEvent {
//         //             event: WindowEvent::CloseRequested,
//         //             ..
//         //         } => *control_flow = ControlFlow::Exit,
//         //         Event::WindowEvent {
//         //             event: WindowEvent::Resized(_),
//         //             ..
//         //         } => {
//         //             //webview.resize(window.hwnd() as *mut _);
//         //         }
//         //         _ => (),
//         //     }
//         // });
//     }
// }



#[derive(Debug)]
pub enum Error {
    InitError,
    NulError(std::ffi::NulError),
    #[cfg(target_os = "windows")]
    WinrtError(winrt::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InitError => "Fail to initialize instance".fmt(f),
            Error::NulError(e) => e.fmt(f),
            #[cfg(target_os = "windows")]
            Error::WinrtError(e) => format!("{:?}", e).fmt(f),
        }
    }
}

impl std::error::Error for Error {}

#[cfg(target_os = "windows")]
impl From<winrt::Error> for Error {
    fn from(error: winrt::Error) -> Self {
        Error::WinrtError(error)
    }
}

impl From<std::ffi::NulError> for Error {
    fn from(error: std::ffi::NulError) -> Self {
        Error::NulError(error)
    }
}


// use winit::{
//     event::{Event, StartCause, WindowEvent},
//     event_loop::{ControlFlow, EventLoop},
//     window::Window,
// };
//use winit::platform::windows::WindowExtWindows;





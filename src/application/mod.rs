#[cfg(not(target_os = "linux"))]
mod general;

use std::{fs::read, path::Path, sync::mpsc::Sender};

#[cfg(not(target_os = "linux"))]
pub use general::*;
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::*;

use crate::{Dispatcher, Result};

use std::marker::PhantomData;

pub struct Callback {
    pub name: String,
    pub function: Box<dyn FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send>,
}

#[derive(Debug, Clone)]
pub struct Icon(pub(crate) Vec<u8>);

impl Icon {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self(read(path)?))
    }

    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Ok(Self(bytes.into()))
    }
}

// TODO complete fields on WindowAttribute
/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct AppWindowAttributes {
    /// Whether the window is resizable or not.
    ///
    /// The default is `true`.
    pub resizable: bool,

    /// The title of the window in the title bar.
    ///
    /// The default is `"wry"`.
    pub title: String,

    /// Whether the window should be maximized upon creation.
    ///
    /// The default is `false`.
    pub maximized: bool,

    /// Whether the window should be immediately visible upon creation.
    ///
    /// The default is `true`.
    pub visible: bool,

    /// Whether the the window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    pub transparent: bool,

    /// Whether the window should have borders and bars.
    ///
    /// The default is `true`.
    pub decorations: bool,

    /// Whether the window should always be on top of other windows.
    ///
    /// The default is `false`.
    pub always_on_top: bool,

    /// The width of the window
    ///
    /// The default is 800.0
    pub width: f64,

    /// The height of the window
    ///
    /// The default is 600.0
    pub height: f64,

    /// The minimum width of the window
    ///
    /// The default is None.
    pub min_width: Option<f64>,

    /// The minimum height of the window
    ///
    /// The default is None.
    pub min_height: Option<f64>,

    /// The maximum width of the window
    ///
    /// The default is None.
    pub max_width: Option<f64>,

    /// The maximum height of the window
    ///
    /// The default is None.
    pub max_height: Option<f64>,

    /// The horizontal position of the window's top left corner
    ///
    /// The default is None.
    pub x: Option<f64>,

    /// The vertical position of the window's top left corner
    ///
    /// The default is None.
    pub y: Option<f64>,

    /// Whether to start the window in fullscreen or not.
    ///
    /// The default is false.
    pub fullscreen: bool,

    /// The window icon.
    ///
    /// The default is None,
    pub icon: Option<Icon>,

    /// Whether to hide the window icon in the taskbar/dock
    ///
    /// The default is false
    pub skip_taskbar: bool,
}

impl Default for AppWindowAttributes {
    #[inline]
    fn default() -> Self {
        Self {
            resizable: true,
            title: "wry".to_owned(),
            maximized: false,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            width: 800.0,
            height: 600.0,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            x: None,
            y: None,
            fullscreen: false,
            icon: None,
            skip_taskbar: false,
        }
    }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WebViewAttributes {
    pub url: Option<String>,
    pub initialization_script: Vec<String>,
}

impl Default for WebViewAttributes {
    #[inline]
    fn default() -> Self {
        Self {
            url: None,
            initialization_script: Vec::default(),
        }
    }
}

pub enum WindowMessage {
    SetResizable(bool),
    SetTitle(String),
    Maximize,
    Unmaximize,
    Minimize,
    Unminimize,
    Show,
    Hide,
    SetTransparent(bool),
    SetDecorations(bool),
    SetAlwaysOnTop(bool),
    SetWidth(f64),
    SetHeight(f64),
    Resize { width: f64, height: f64 },
    SetMinSize { min_width: f64, min_height: f64 },
    SetMaxSize { max_width: f64, max_height: f64 },
    SetX(f64),
    SetY(f64),
    SetPosition { x: f64, y: f64 },
    SetFullscreen(bool),
    SetIcon(Icon),
}

pub enum WebviewMessage {
    EvalScript(String),
}

pub enum AppMessage {
    NewWindow(
        AppWindowAttributes,
        WebViewAttributes,
        Option<Vec<Callback>>,
        Sender<WindowId>,
    ),
}

pub enum Message<I, T> {
    Webview(I, WebviewMessage),
    Window(I, WindowMessage),
    App(AppMessage),
    Custom(T),
}

pub trait ApplicationDispatcher<I, T> {
    fn dispatch_message(&self, message: Message<I, T>) -> Result<()>;
    fn add_window(
        &self,
        window_attrs: AppWindowAttributes,
        webview_attrs: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<WindowId>;
}

pub struct WebviewDispatcher<I, T, D>(D, I, PhantomData<T>);

impl<I: Copy, T, D: ApplicationDispatcher<I, T>> WebviewDispatcher<I, T, D> {
    fn new(dispatcher: D, window_id: I) -> Self {
        Self(dispatcher, window_id, PhantomData)
    }

    pub fn eval_script<S: Into<String>>(&self, script: S) -> Result<()> {
        self.0.dispatch_message(Message::Webview(
            self.1,
            WebviewMessage::EvalScript(script.into()),
        ))
    }
}

pub struct WindowDispatcher<I, T, D>(D, I, PhantomData<T>);

impl<I: Copy, T, D: ApplicationDispatcher<I, T>> WindowDispatcher<I, T, D> {
    fn new(dispatcher: D, window_id: I) -> Self {
        Self(dispatcher, window_id, PhantomData)
    }

    pub fn set_resizable(&self, resizable: bool) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetResizable(resizable),
        ))
    }

    pub fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetTitle(title.into()),
        ))
    }

    pub fn maximize(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Maximize))
    }
    pub fn unmaximize(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Unmaximize))
    }

    pub fn minimize(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Minimize))
    }

    pub fn unminimize(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Unminimize))
    }

    pub fn show(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Show))
    }

    pub fn hide(&self) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::Hide))
    }

    pub fn set_transparent(&self, resizable: bool) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetResizable(resizable),
        ))
    }

    pub fn set_decorations(&self, decorations: bool) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetResizable(decorations),
        ))
    }

    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetAlwaysOnTop(always_on_top),
        ))
    }

    pub fn set_width(&self, width: f64) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetWidth(width)))
    }

    pub fn set_height(&self, height: f64) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetHeight(height)))
    }

    pub fn resize(&self, width: f64, height: f64) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::Resize { width, height },
        ))
    }

    pub fn set_min_size(&self, min_width: f64, min_height: f64) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetMinSize {
                min_width,
                min_height,
            },
        ))
    }

    pub fn set_max_size(&self, max_width: f64, max_height: f64) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetMaxSize {
                max_width,
                max_height,
            },
        ))
    }

    pub fn set_x(&self, x: f64) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetX(x)))
    }

    pub fn set_y(&self, y: f64) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetY(y)))
    }

    pub fn set_position(&self, x: f64, y: f64) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetPosition { x, y }))
    }

    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetFullscreen(fullscreen),
        ))
    }

    pub fn set_icon(&self, icon: Icon) -> Result<()> {
        self.0
            .dispatch_message(Message::Window(self.1, WindowMessage::SetIcon(icon)))
    }
}

pub trait ApplicationExt<'a, T>: Sized {
    type Dispatcher: ApplicationDispatcher<Self::Id, T>;
    type Id: Copy;

    fn new() -> Result<Self>;

    fn create_webview(
        &mut self,
        window_attribures: AppWindowAttributes,
        webview_attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<Self::Id>;
    fn set_message_handler<F: FnMut(T) + 'static>(&mut self, handler: F);

    fn dispatcher(&self) -> Self::Dispatcher;

    fn window_dispatcher(
        &self,
        window_id: Self::Id,
    ) -> WindowDispatcher<Self::Id, T, Self::Dispatcher> {
        WindowDispatcher::new(self.dispatcher(), window_id)
    }

    fn webview_dispatcher(
        &self,
        window_id: Self::Id,
    ) -> WebviewDispatcher<Self::Id, T, Self::Dispatcher> {
        WebviewDispatcher::new(self.dispatcher(), window_id)
    }

    fn run(self);
}

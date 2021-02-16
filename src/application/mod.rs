#[cfg(not(target_os = "linux"))]
mod general;
#[cfg(not(target_os = "linux"))]
use general::Application as InnerApplication;
#[cfg(not(target_os = "linux"))]
pub use general::{AppDispatcher, WindowId};
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
use gtkrs::Application as InnerApplication;
#[cfg(target_os = "linux")]
pub use gtkrs::{AppDispatcher, WindowId};

use crate::{Dispatcher, Result};

use std::{fs::read, marker::PhantomData, path::Path, sync::mpsc::Sender};

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

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WebViewAttributes {
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

    pub url: Option<String>,
    pub initialization_scripts: Vec<String>,
}

impl WebViewAttributes {
    fn split(self) -> (AppWindowAttributes, AppWebViewAttributes) {
        (
            AppWindowAttributes {
                resizable: self.resizable,
                title: self.title,
                maximized: self.maximized,
                visible: self.visible,
                transparent: self.transparent,
                decorations: self.decorations,
                always_on_top: self.always_on_top,
                width: self.width,
                height: self.height,
                min_width: self.min_width,
                min_height: self.min_height,
                max_width: self.max_width,
                max_height: self.max_height,
                x: self.x,
                y: self.y,
                fullscreen: self.fullscreen,
                icon: self.icon,
                skip_taskbar: self.skip_taskbar,
            },
            AppWebViewAttributes {
                url: self.url,
                initialization_scripts: self.initialization_scripts,
            },
        )
    }
}

impl Default for WebViewAttributes {
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
            url: None,
            initialization_scripts: vec![],
        }
    }
}

struct AppWindowAttributes {
    pub resizable: bool,
    pub title: String,
    pub maximized: bool,
    pub visible: bool,
    pub transparent: bool,
    pub decorations: bool,
    pub always_on_top: bool,
    pub width: f64,
    pub height: f64,
    pub min_width: Option<f64>,
    pub min_height: Option<f64>,
    pub max_width: Option<f64>,
    pub max_height: Option<f64>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub fullscreen: bool,
    pub icon: Option<Icon>,
    pub skip_taskbar: bool,
}

struct AppWebViewAttributes {
    pub url: Option<String>,
    pub initialization_scripts: Vec<String>,
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

pub enum Message<T> {
    Webview(WindowId, WebviewMessage),
    Window(WindowId, WindowMessage),
    NewWindow(WebViewAttributes, Option<Vec<Callback>>, Sender<WindowId>),
    Custom(T),
}

pub trait ApplicationDispatcher<T> {
    fn dispatch_message(&self, message: Message<T>) -> Result<()>;
    fn add_window(
        &self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<WindowId>;
}

pub struct WebviewDispatcher<T, D>(D, WindowId, PhantomData<T>);

impl<T, D: ApplicationDispatcher<T>> WebviewDispatcher<T, D> {
    fn new(dispatcher: D, window_id: WindowId) -> Self {
        Self(dispatcher, window_id, PhantomData)
    }

    pub fn eval_script<S: Into<String>>(&self, script: S) -> Result<()> {
        self.0.dispatch_message(Message::Webview(
            self.1,
            WebviewMessage::EvalScript(script.into()),
        ))
    }
}

pub struct WindowDispatcher<T, D>(D, WindowId, PhantomData<T>);

impl<T, D: ApplicationDispatcher<T>> WindowDispatcher<T, D> {
    fn new(dispatcher: D, window_id: WindowId) -> Self {
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

pub struct Application {
    inner: InnerApplication<()>,
}

impl Application {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: InnerApplication::new()?,
        })
    }

    pub fn create_webview(
        &mut self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<WindowId> {
        self.inner.create_webview(attributes, callbacks)
    }

    pub fn set_message_handler<F: FnMut(()) + 'static>(&mut self, handler: F) {
        self.inner.set_message_handler(handler)
    }

    pub fn dispatcher(&self) -> AppDispatcher<()> {
        self.inner.dispatcher()
    }

    pub fn window_dispatcher(
        &self,
        window_id: WindowId,
    ) -> WindowDispatcher<(), AppDispatcher<()>> {
        WindowDispatcher::new(self.dispatcher(), window_id)
    }

    pub fn webview_dispatcher(
        &self,
        window_id: WindowId,
    ) -> WebviewDispatcher<(), AppDispatcher<()>> {
        WebviewDispatcher::new(self.dispatcher(), window_id)
    }

    pub fn run(self) {
        self.inner.run()
    }
}

trait ApplicationExt<'a, T>: Sized {
    type Dispatcher: ApplicationDispatcher<T>;
    type Id: Copy;

    fn new() -> Result<Self>;

    fn create_webview(
        &mut self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<Self::Id>;
    fn set_message_handler<F: FnMut(T) + 'static>(&mut self, handler: F);

    fn dispatcher(&self) -> Self::Dispatcher;

    fn run(self);
}

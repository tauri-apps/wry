#[cfg(not(target_os = "linux"))]
mod general;

use std::{fs::read, path::Path};

#[cfg(not(target_os = "linux"))]
pub use general::*;
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::*;

#[cfg(target_os = "linux")]
pub use gtkrs::GtkWindow as Window;

#[cfg(not(target_os = "linux"))]
pub use general::WinitWindow as Window;

use crate::{Dispatcher, Result};

use std::marker::PhantomData;

#[cfg(not(target_os = "linux"))]
use winit::{
    dpi::{LogicalSize, Size},
    window::{Fullscreen, WindowAttributes},
};

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
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl From<&AppWindowAttributes> for WindowAttributes {
    fn from(w: &AppWindowAttributes) -> Self {
        let min_inner_size = match (w.min_width, w.min_height) {
            (Some(min_width), Some(min_height)) => {
                Some(Size::from(LogicalSize::new(min_width, min_height)))
            }
            _ => None,
        };

        let max_inner_size = match (w.max_width, w.max_height) {
            (Some(max_width), Some(max_height)) => {
                Some(Size::from(LogicalSize::new(max_width, max_height)))
            }
            _ => None,
        };

        let fullscreen = if w.fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        };

        Self {
            resizable: w.resizable,
            title: w.title.clone(),
            maximized: w.maximized,
            visible: w.visible,
            transparent: w.transparent,
            decorations: w.decorations,
            always_on_top: w.always_on_top,
            inner_size: Some(Size::from(LogicalSize::new(w.width, w.height))),
            min_inner_size,
            max_inner_size,
            fullscreen,
            ..Default::default()
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
    SetLocation { x: f64, y: f64 },
    SetFullscreen(bool),
    SetIcon(Icon),
}

pub enum WebviewMessage {
    EvalScript(String),
}

pub enum Message<I, T> {
    Webview(I, WebviewMessage),
    Window(I, WindowMessage),
    Custom(T),
}

pub trait ApplicationDispatcher<I, T> {
    fn dispatch_message(&self, message: Message<I, T>) -> Result<()>;
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

    pub fn set_window_title<S: Into<String>>(&self, title: S) -> Result<()> {
        self.0.dispatch_message(Message::Window(
            self.1,
            WindowMessage::SetTitle(title.into()),
        ))
    }
}

pub trait ApplicationExt<'a, T>: Sized {
    type Window: WindowExt<'a>;
    type Dispatcher: ApplicationDispatcher<
        <<Self as ApplicationExt<'a, T>>::Window as WindowExt<'a>>::Id,
        T,
    >;

    fn new() -> Result<Self>;

    fn create_window(&self, attributes: AppWindowAttributes) -> Result<Self::Window>;

    fn create_webview(
        &mut self,
        window: Self::Window,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<()>;
    fn set_message_handler<F: FnMut(T) + 'static>(&mut self, handler: F);

    fn dispatcher(&self) -> Self::Dispatcher;

    fn window_dispatcher(
        &self,
        window_id: <<Self as ApplicationExt<'a, T>>::Window as WindowExt<'a>>::Id,
    ) -> WindowDispatcher<
        <<Self as ApplicationExt<'a, T>>::Window as WindowExt<'a>>::Id,
        T,
        Self::Dispatcher,
    > {
        WindowDispatcher::new(self.dispatcher(), window_id)
    }

    fn webview_dispatcher(
        &self,
        window_id: <<Self as ApplicationExt<'a, T>>::Window as WindowExt<'a>>::Id,
    ) -> WebviewDispatcher<
        <<Self as ApplicationExt<'a, T>>::Window as WindowExt<'a>>::Id,
        T,
        Self::Dispatcher,
    > {
        WebviewDispatcher::new(self.dispatcher(), window_id)
    }

    fn run(self);
}

pub trait WindowExt<'a> {
    type Id: Copy;
    fn id(&self) -> Self::Id;
}

#[cfg(not(target_os = "linux"))]
mod general;
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

#[cfg(not(target_os = "linux"))]
use winit::{
    dpi::{LogicalSize, Size},
    window::WindowAttributes,
};

pub struct Callback {
    pub name: String,
    pub function: Box<dyn FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send>,
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

    /// The horizontal position of the window's top left cornet
    ///
    /// The default is None.
    pub x: Option<f64>,

    /// The vertical position of the window's top left cornet
    ///
    /// The default is None.
    pub y: Option<f64>,
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
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl From<&AppWindowAttributes> for WindowAttributes {
    fn from(w: &AppWindowAttributes) -> Self {
        let min_inner_size = if w.min_width.is_some() && w.min_height.is_some() {
            Some(Size::from(LogicalSize::new(
                w.min_width.unwrap(),
                w.min_height.unwrap(),
            )))
        } else {
            None
        };

        let max_inner_size = if w.max_width.is_some() && w.max_height.is_some() {
            Some(Size::from(LogicalSize::new(
                w.max_width.unwrap(),
                w.max_height.unwrap(),
            )))
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

pub enum Message<I, T> {
    Script(I, String),
    Custom(T),
}

pub trait ApplicationDispatcher<I, T> {
    fn dispatch_message(&self, message: Message<I, T>) -> Result<()>;
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
    fn run(self);
}

pub trait WindowExt<'a> {
    type Id;
    fn id(&self) -> Self::Id;
}

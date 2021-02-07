#[cfg(not(target_os = "linux"))]
mod general;
#[cfg(not(target_os = "linux"))]
pub use general::*;
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::*;

use crate::{Dispatcher, Result};

#[cfg(not(target_os = "linux"))]
use winit::window::WindowAttributes;

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
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl From<&AppWindowAttributes> for WindowAttributes {
    fn from(w: &AppWindowAttributes) -> Self {
        Self {
            resizable: w.resizable,
            title: w.title.clone(),
            maximized: w.maximized,
            visible: w.visible,
            transparent: w.transparent,
            decorations: w.decorations,
            always_on_top: w.always_on_top,
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
        indow: Self::Window,
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

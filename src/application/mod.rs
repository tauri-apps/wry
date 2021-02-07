#[cfg(not(target_os = "linux"))]
mod general;
#[cfg(not(target_os = "linux"))]
pub use general::*;
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::*;

use crate::Dispatcher;

use winit::window::WindowAttributes;

pub struct Callback {
    pub name: String,
    pub function: Box<dyn FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send>,
}

// TODO complete fields on WindowAttribute
/// Attributes to use when creating a webview window.
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

    pub url: Option<String>,

    pub initialization_script: Vec<String>,
}

impl Default for WebViewAttributes {
    #[inline]
    fn default() -> WebViewAttributes {
        WebViewAttributes {
            resizable: true,
            title: "wry".to_owned(),
            maximized: false,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            url: None,
            initialization_script: Vec::default(),
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl From<&WebViewAttributes> for WindowAttributes {
    fn from(w: &WebViewAttributes) -> Self {
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

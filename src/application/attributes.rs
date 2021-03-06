use crate::{Result, RpcRequest, RpcResponse, WindowProxy};

use std::{fs::read, path::Path};

#[cfg(feature = "file-drop")]
use crate::FileDropHandler;

pub type WindowRpcHandler = Box<dyn Fn(WindowProxy, RpcRequest) -> Option<RpcResponse> + Send>;

pub struct CustomProtocol {
    pub name: String,
    pub handler: Box<dyn Fn(&str) -> Result<Vec<u8>> + Send>,
}

///	An icon used for the window title bar, taskbar, etc.
#[derive(Debug, Clone)]
pub struct Icon(pub(crate) Vec<u8>);

impl Icon {
    /// Creates an icon from the file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Ok(Self(read(path)?))
    }
    /// Creates an icon from raw bytes.
    pub fn from_bytes<B: Into<Vec<u8>>>(bytes: B) -> Result<Self> {
        Ok(Self(bytes.into()))
    }
}

/// Attributes to use when creating a webview window.
#[derive(Debug, Clone)]
pub struct Attributes {
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

    /// Whether the WebView window should be transparent. If this is true, writing colors
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

    /// The width of the window.
    ///
    /// The default is `800.0`.
    pub width: f64,

    /// The height of the window.
    ///
    /// The default is `600.0`.
    pub height: f64,

    /// The minimum width of the window.
    ///
    /// The default is `None`.
    pub min_width: Option<f64>,

    /// The minimum height of the window.
    ///
    /// The default is `None`.
    pub min_height: Option<f64>,

    /// The maximum width of the window.
    ///
    /// The default is `None`.
    pub max_width: Option<f64>,

    /// The maximum height of the window.
    ///
    /// The default is `None`.
    pub max_height: Option<f64>,

    /// The horizontal position of the window's top left corner.
    ///
    /// The default is `None`.
    pub x: Option<f64>,

    /// The vertical position of the window's top left corner.
    ///
    /// The default is `None`.
    pub y: Option<f64>,

    /// Whether to start the window in fullscreen or not.
    ///
    /// The default is `false`.
    pub fullscreen: bool,

    /// The window icon.
    ///
    /// The default is `None`.
    pub icon: Option<Icon>,

    /// Whether to hide the window icon in the taskbar/dock.
    ///
    /// The default is `false`
    pub skip_taskbar: bool,

    /// The URL to be loaded in the webview window.
    ///
    /// The default is `None`.
    pub url: Option<String>,

    /// Javascript Code to be initialized when loading new pages.
    ///
    /// The default is an empty vector.
    pub initialization_scripts: Vec<String>,

    /// A closure that will be executed when a file is dropped on the window.
    ///
    /// The default is `None`.
    #[cfg(feature = "file-drop")]
    pub file_drop_handler: Option<FileDropHandler>,
}

impl Attributes {
    pub(crate) fn split(self) -> (InnerWindowAttributes, InnerWebViewAttributes) {
        (
            InnerWindowAttributes {
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
            InnerWebViewAttributes {
                transparent: self.transparent,
                url: self.url,
                initialization_scripts: self.initialization_scripts,

                #[cfg(feature = "file-drop")]
                file_drop_handler: self.file_drop_handler
            },
        )
    }
}

impl Default for Attributes {
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

            #[cfg(feature = "file-drop")]
            file_drop_handler: None,
        }
    }
}

pub(crate) struct InnerWindowAttributes {
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

pub(crate) struct InnerWebViewAttributes {
    pub transparent: bool,
    pub url: Option<String>,
    pub initialization_scripts: Vec<String>,

    #[cfg(feature = "file-drop")]
    pub file_drop_handler: Option<FileDropHandler>,
}

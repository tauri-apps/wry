use core::fmt;
use std::sync::{Arc, RwLock};

#[cfg(not(target_os = "linux"))]
mod general;
#[cfg(not(target_os = "linux"))]
pub use general::WindowId;
#[cfg(not(target_os = "linux"))]
use general::{InnerApplication, InnerApplicationProxy};
#[cfg(target_os = "linux")]
mod gtkrs;
#[cfg(target_os = "linux")]
pub use gtkrs::WindowId;
#[cfg(target_os = "linux")]
use gtkrs::{InnerApplication, InnerApplicationProxy};

use crate::{Result, RpcHandler};

use std::{fs::read, path::Path, sync::mpsc::Sender, path::PathBuf, cell::Cell};

use serde_json::Value;

/// Defines a Rust callback function which can be called on Javascript side.
pub struct Callback {
    /// Name of the callback function.
    pub name: String,
    /// The function itself takes three parameters and return a number as return value.
    ///
    /// [`WindowProxy`] can let you adjust the corresponding WebView window.
    ///
    /// The second parameter `i32` is a sequence number to count how many times this function has
    /// been called.
    ///
    /// The last vector is the actual list of arguments passed from the caller.
    ///
    /// The return value of the function is a number. Return `0` indicates the call is successful,
    /// and return others if not.
    pub function: Box<dyn FnMut(WindowProxy, i32, Vec<Value>) -> Result<()> + Send>,
}

impl std::fmt::Debug for Callback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Callback")
            .field("name", &self.name)
            .finish()
    }
}

pub struct CustomProtocol {
    pub name: String,
    pub handler: Box<dyn Fn(&str) -> Result<Vec<u8>> + Send>,
}

impl std::fmt::Debug for CustomProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CustomProtocol")
            .field("name", &self.name)
            .finish()
    }
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
    pub file_drop_handler: Option<FileDropHandler>,
}

impl Attributes {
    fn split(self) -> (InnerWindowAttributes, InnerWebViewAttributes) {
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
                file_drop_handler: self.file_drop_handler,
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
            file_drop_handler: None,
        }
    }
}

struct InnerWindowAttributes {
    resizable: bool,
    title: String,
    maximized: bool,
    visible: bool,
    transparent: bool,
    decorations: bool,
    always_on_top: bool,
    width: f64,
    height: f64,
    min_width: Option<f64>,
    min_height: Option<f64>,
    max_width: Option<f64>,
    max_height: Option<f64>,
    x: Option<f64>,
    y: Option<f64>,
    fullscreen: bool,
    icon: Option<Icon>,
    skip_taskbar: bool,
}

struct InnerWebViewAttributes {
    transparent: bool,
    url: Option<String>,
    initialization_scripts: Vec<String>,
    file_drop_handler: Option<FileDropHandler>,
}

/// Describes a message for a WebView window.
#[derive(Debug)]
pub enum WindowMessage {
    SetResizable(bool),
    SetTitle(String),
    Maximize,
    Unmaximize,
    Minimize,
    Unminimize,
    Show,
    Hide,
    Close,
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
    EvaluationScript(String),
}

/// Describes a general message.
#[derive(Debug)]
pub enum Message {
    Window(WindowId, WindowMessage),
    NewWindow(
        Attributes,
        Option<Vec<Callback>>,
        Sender<WindowId>,
        Option<CustomProtocol>,
    ),
}

/// A proxy to sent custom messages to [`Application`].
///
/// This can be created by calling [`Application::application_proxy`].
#[derive(Clone)]
pub struct ApplicationProxy {
    inner: InnerApplicationProxy,
}

impl ApplicationProxy {
    /// Sends a message to the [`Application`] from which this proxy was created.
    ///
    /// Returns an Err if the associated EventLoop no longer exists.
    pub fn send_message(&self, message: Message) -> Result<()> {
        self.inner.send_message(message)
    }
    /// Adds another WebView window to the application. Returns its [`WindowProxy`] after created.
    pub fn add_window(&self, attributes: Attributes) -> Result<WindowProxy> {
        let id = self.inner.add_window(attributes, None, None)?;
        Ok(WindowProxy::new(self.clone(), id))
    }

    /// Adds another WebView window to the application with more configuration options. Returns its [`WindowProxy`] after created.
    pub fn add_window_with_configs(
        &self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
        custom_protocol: Option<CustomProtocol>,
    ) -> Result<WindowProxy> {
        let id = self
            .inner
            .add_window(attributes, callbacks, custom_protocol)?;
        Ok(WindowProxy::new(self.clone(), id))
    }
}

trait AppProxy {
    fn send_message(&self, message: Message) -> Result<()>;
    fn add_window(
        &self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
        custom_protocol: Option<CustomProtocol>,
        //rpc_handler: Option<RpcHandler>,
    ) -> Result<WindowId>;
}

/// A proxy to customize its corresponding WebView window.
///
/// Whenever [`Application::add_window`] creates a WebView Window, it will return this for you. But
/// it can still be retrieved from [`Application::window_proxy`] in case you drop the window proxy
/// too early.
pub struct WindowProxy {
    proxy: ApplicationProxy,
    id: WindowId,
}

impl WindowProxy {
    fn new(proxy: ApplicationProxy, id: WindowId) -> Self {
        Self { proxy, id }
    }

    /// Gets the id of the WebView window.
    pub fn id(&self) -> WindowId {
        self.id
    }

    pub fn set_resizable(&self, resizable: bool) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetResizable(resizable),
        ))
    }

    pub fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetTitle(title.into()),
        ))
    }

    pub fn maximize(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Maximize))
    }
    pub fn unmaximize(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Unmaximize))
    }

    pub fn minimize(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Minimize))
    }

    pub fn unminimize(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Unminimize))
    }

    pub fn show(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Show))
    }

    pub fn hide(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Hide))
    }

    pub fn close(&self) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::Close))
    }

    pub fn set_decorations(&self, decorations: bool) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetDecorations(decorations),
        ))
    }

    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetAlwaysOnTop(always_on_top),
        ))
    }

    pub fn set_width(&self, width: f64) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::SetWidth(width)))
    }

    pub fn set_height(&self, height: f64) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::SetHeight(height)))
    }

    pub fn resize(&self, width: f64, height: f64) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::Resize { width, height },
        ))
    }

    pub fn set_min_size(&self, min_width: f64, min_height: f64) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetMinSize {
                min_width,
                min_height,
            },
        ))
    }

    pub fn set_max_size(&self, max_width: f64, max_height: f64) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetMaxSize {
                max_width,
                max_height,
            },
        ))
    }

    pub fn set_x(&self, x: f64) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::SetX(x)))
    }

    pub fn set_y(&self, y: f64) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::SetY(y)))
    }

    pub fn set_position(&self, x: f64, y: f64) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetPosition { x, y },
        ))
    }

    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::SetFullscreen(fullscreen),
        ))
    }

    pub fn set_icon(&self, icon: Icon) -> Result<()> {
        self.proxy
            .send_message(Message::Window(self.id, WindowMessage::SetIcon(icon)))
    }

    pub fn evaluate_script<S: Into<String>>(&self, script: S) -> Result<()> {
        self.proxy.send_message(Message::Window(
            self.id,
            WindowMessage::EvaluationScript(script.into()),
        ))
    }
}

/// Provides a way to create and manage WebView windows.
///
/// Application is the main gateway of all WebView windows. You can simply call
/// [`Application::add_window`] to create a WebView embedded in a window and delegate to
/// [`Application`].
///
/// [`Application::run`] has to be called on the (main) thread who creates its [`Application`]. In
/// order to interact with application from other threads, [`Application::application_proxy`]
/// and [`Application::window_proxy`] allow you to retrieve their proxies for further management
/// when running the application.
pub struct Application {
    inner: InnerApplication,
}

impl Application {
    /// Builds a new application.
    ///
    /// ***For cross-platform compatibility, the [`Application`] must be created on the main thread.***
    /// Attempting to create the application on a different thread will usually result in unexpected
    /// behaviors and even panic. This restriction isn't strictly necessary on all platforms, but is
    /// imposed to eliminate any nasty surprises when porting to platforms that require it.
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: InnerApplication::new()?,
            //rpc_handler: None,
        })
    }

    /// Adds a WebView window to the application. Returns its [`WindowProxy`] after created.
    ///
    /// [`Attributes`] is the configuration struct for you to customize the window.
    ///
    /// To create a default window, you could just pass `.add_window(Default::default(), None)`.
    pub fn add_window(&mut self, attributes: Attributes) -> Result<WindowProxy> {
        let id = self.inner.create_webview(attributes, None, None)?;
        Ok(self.window_proxy(id))
    }

    /// Adds a WebView window to the application with more configuration options. Returns its [`WindowProxy`] after created.
    ///
    /// [`Attributes`] is the configuration struct for you to customize the window.
    ///
    /// [`Callback`] allows you to define rust function to be called on Javascript side for its window.
    ///
    /// [`CustomProtocol`] allows you to define custom URL scheme to handle actions like loading
    /// assets.
    ///
    /// To create a default window, you could just pass `.add_window(Default::default(), None)`.
    pub fn add_window_with_configs(
        &mut self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
        custom_protocol: Option<CustomProtocol>,
    ) -> Result<WindowProxy> {
        let id = self
            .inner
            .create_webview(attributes, callbacks, custom_protocol)?;
        Ok(self.window_proxy(id))
    }

    /// Returns a [`ApplicationProxy`] for you to manage the application from other threads.
    pub fn application_proxy(&self) -> ApplicationProxy {
        ApplicationProxy {
            inner: self.inner.application_proxy(),
            //rpc_handler: self.inner.
        }
    }

    /// Returns the [`WindowProxy`] with given `WindowId`.
    pub fn window_proxy(&self, window_id: WindowId) -> WindowProxy {
        WindowProxy::new(self.application_proxy(), window_id)
    }

    /// Set an RPC message handler.
    pub fn set_rpc_handler(&mut self, handler: RpcHandler) {
        // TODO: detect if webviews already exist and panic
        // TODO: because this should be set before callling add_window().

        self.inner.rpc_handler = Some(Arc::new(handler));
    }

    /// Set a file drop handler.
    pub fn set_file_drop_handler(&mut self, handler: FileDropHandler) {
        self.inner.file_drop_handler = Some(handler);
    }

    /// Consume the application and start running it. This will hijack the main thread and iterate
    /// its event loop. To further control the application after running, [`ApplicationProxy`] and
    /// [`WindowProxy`] allow you to do so on other threads.
    pub fn run(self) {
        self.inner.run()
    }
}

trait App: Sized {
    type Proxy: AppProxy;
    type Id: Copy;

    fn new() -> Result<Self>;

    fn create_webview(
        &mut self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
        custom_protocol: Option<CustomProtocol>,
        //rpc_handler: Option<RpcHandler>,
    ) -> Result<Self::Id>;

    fn application_proxy(&self) -> Self::Proxy;

    fn run(self);
}

pub(crate) const RPC_CALLBACK_NAME: &str = "__rpc__";
const RPC_VERSION: &str = "2.0";

/// Function call from Javascript.
///
/// If the callback name matches the name for an RPC handler
/// the payload should be passed to the handler transparently.
///
/// Otherwise attempt to find a `Callback` with the same name
/// and pass it the payload `params`.
#[derive(Debug, Serialize, Deserialize)]
pub struct FuncCall {
    pub(crate) callback: String,
    pub(crate) payload: RpcRequest,
}

/// RPC request message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcRequest {
    jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// RPC response message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RpcResponse {
    jsonrpc: String,
    pub(crate) id: Option<Value>,
    pub(crate) result: Option<Value>,
    pub(crate) error: Option<Value>,
}

impl RpcResponse {

    /// Create a new result response.
    pub fn new_result(id: Option<Value>, result: Option<Value>) -> Self {
        Self {
            jsonrpc: RPC_VERSION.to_string(),
            id, result,
            error: None
        } 
    }

    /// Create a new error response.
    pub fn new_error(id: Option<Value>, error: Option<Value>) -> Self {
        Self {
            jsonrpc: RPC_VERSION.to_string(),
            id, error,
            result: None
        } 
    }
}

#[derive(Debug, Serialize, Clone)]
/// The status of a file drop event.
pub enum FileDropStatus {
    /// The file has been dragged onto the window, but has not been dropped yet.
    Hovered(PathBuf),

    /// The file has been dropped onto the window.
    Dropped(PathBuf),

    /// The file drop has been aborted.
    Cancelled(PathBuf),
}

/// This needs to be defined because internally the respective events do not always yield a PathBuf.
/// We can easily remember what was cancelled though, as Hovered and Dropped events will always yield a PathBuf which we will save ourselves for later reference.
pub(crate) enum FileDropEvent {
    Hovered,
    Dropped,
    Cancelled
}

#[derive(Clone)]
pub struct FileDropHandler {
    f: Arc<Box<dyn Fn(FileDropStatus) + Send + Sync + 'static>>
}
impl FileDropHandler {
    /// Initializes a new file drop handler.
    /// Example: FileDropHandler:new(|status: FileDropStatus| ...)
    pub fn new<T>(f: T) -> FileDropHandler
    where
        T: Fn(FileDropStatus) + Send + Sync + 'static
    {
        FileDropHandler { f: Arc::new(Box::new(f)) }
    }

    pub fn call(&self, status: FileDropStatus) {
        (self.f)(status);
    }
}
impl fmt::Debug for FileDropHandler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FileDropHandler")
    }
}

pub(crate) struct FileDropController {
    pub(crate) handlers: (Option<FileDropHandler>, Option<FileDropHandler>),
    pub(crate) active_file_drop: Cell<Option<FileDropStatus>>
}
impl FileDropController {
    pub(crate) fn new(handlers: (Option<FileDropHandler>, Option<FileDropHandler>)) -> FileDropController {
        debug_assert!(handlers.0.is_some() || handlers.1.is_some(), "Tried to create a FileDropController with no file drop handlers!");
        FileDropController {
            handlers,
            active_file_drop: Cell::new(None)
        }
    }

    /// Called when a file drop event occurs. Bubbles the event up to the handler.
    pub(crate) fn file_drop(&self, event: FileDropEvent, path: Option<PathBuf>) {
        let new_status;
        match event {
            FileDropEvent::Hovered => {
                new_status = FileDropStatus::Hovered(path.unwrap());
            },
            FileDropEvent::Dropped => {
                new_status = FileDropStatus::Dropped(path.unwrap());
            },
            FileDropEvent::Cancelled => {
                // Remember the path of what was aborted
                if let Some(status) = self.active_file_drop.take() {
                    let path = match status {
                        FileDropStatus::Hovered(path) => { path }
                        FileDropStatus::Dropped(path) => { path }
                        FileDropStatus::Cancelled(path) => { path }
                    };
                    self.call(FileDropStatus::Cancelled(path));
                }
                return;
            }
        }

        self.active_file_drop.set(Some(new_status.clone()));
        self.call(new_status);
    }

    fn call(&self, status: FileDropStatus) {
        // Kind of silly, but the most memory efficient
        match self.handlers.0 {
            Some(ref webview_file_drop_handler) => {
                match self.handlers.1 {
                    Some(ref app_file_drop_handler) => {
                        webview_file_drop_handler.call(status.clone());
                        app_file_drop_handler.call(status);
                    },
                    None => webview_file_drop_handler.call(status)
                }
            },
            None => {
                match self.handlers.1 {
                    Some(ref app_file_drop_handler) => app_file_drop_handler.call(status),
                    None => {}
                }
            }
        }
    }
}

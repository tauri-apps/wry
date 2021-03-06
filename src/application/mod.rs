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

use crate::file_drop::FileDropHandler;

mod attributes;
pub use attributes::{Attributes, CustomProtocol, Icon, WindowRpcHandler};
pub(crate) use attributes::{InnerWebViewAttributes, InnerWindowAttributes};

use crate::Result;

use std::sync::mpsc::Sender;

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
pub enum Message {
    Window(WindowId, WindowMessage),
    NewWindow(
        Attributes,
        Sender<WindowId>,
        Option<WindowRpcHandler>,
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
        rpc_handler: Option<WindowRpcHandler>,
        custom_protocol: Option<CustomProtocol>,
    ) -> Result<WindowProxy> {
        let id = self
            .inner
            .add_window(attributes, rpc_handler, custom_protocol)?;
        Ok(WindowProxy::new(self.clone(), id))
    }
}

trait AppProxy {
    fn send_message(&self, message: Message) -> Result<()>;
    fn add_window(
        &self,
        attributes: Attributes,
        rpc_handler: Option<WindowRpcHandler>,
        custom_protocol: Option<CustomProtocol>,
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
        handler: Option<WindowRpcHandler>,
        custom_protocol: Option<CustomProtocol>,
    ) -> Result<WindowProxy> {
        let id = self
            .inner
            .create_webview(attributes, handler, custom_protocol)?;
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
        rpc_handler: Option<WindowRpcHandler>,
        custom_protocol: Option<CustomProtocol>,
    ) -> Result<Self::Id>;

    fn application_proxy(&self) -> Self::Proxy;

    fn run(self);
}

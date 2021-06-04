//! A builder that wraps both [`WindowBuilder`] and [`WebViewBuilder`].

use crate::{
  application::{
    window::{Window, WindowBuilder},
    Application,
  },
  webview::{Dispatcher, FileDropEvent, RpcRequest, RpcResponse, WebView, WebViewBuilder},
};

use tao::{
  dpi::{Position, Size},
  event_loop::EventLoopWindowTarget,
  menu::Menu,
  window::{Fullscreen, Icon},
};

macro_rules! window_builder {
  (
    $(#[$meta:meta])+
    method => $method:ident,
    original => $original:ident,
    $(
      arg => $arg:ident: $type:path,
      $(generic => $generic:path)?
    )?
  ) => {
    $(#[$meta])+
    #[doc = ""]
    #[doc = "_**Note:** if the [`Builder`] was created with [`Builder::with_window`] then this method will have no effect._"]
    #[inline]
    pub fn $method $($(<T: $generic>)?)? (mut self $(, $arg: $type)? ) -> Self {
      if let BuilderWindowBuilder::Builder { builder, event_loop } = self.window {
        self.window = BuilderWindowBuilder::Builder {
          builder: builder.$original($($arg)?),
          event_loop
        };
      }

      self
    }
  };
}

/// lol what do i call this
enum BuilderWindowBuilder<'event, Event: 'static> {
  Window(Window),
  Builder {
    builder: WindowBuilder,
    event_loop: &'event EventLoopWindowTarget<Event>,
  },
}

/// A streamlined builder to create a [`WebView`](crate::webview::WebView).
///
/// You can instead use [`WindowBuilder`] and [`WebViewBuilder`] if you wish to separate the
/// builders.
pub struct Builder<'event, Event: 'static> {
  window: BuilderWindowBuilder<'event, Event>,
  webview: WebViewBuilder,
}

impl<'event, Event: 'static> Builder<'event, Event> {
  /// Create a new [`Builder`] attached to an existing [`EventLoop`](tao::event_loop::EventLoop).
  #[inline]
  pub fn new(event_loop: &'event EventLoopWindowTarget<Event>) -> Self {
    Builder {
      window: BuilderWindowBuilder::Builder {
        builder: WindowBuilder::new(),
        event_loop,
      },
      webview: WebViewBuilder::new(),
    }
  }

  /// Create a new [`Builder`] with an already built [`Window`].
  ///
  /// You should still have the [`EventLoop`] you created the [`Window`] with if you want to control
  /// the [`WebView`] resulting from this [`Builder`].
  ///
  /// [`EventLoop`]: tao::event_loop::EventLoop
  #[inline]
  pub fn with_window(window: Window) -> Self {
    Self {
      window: BuilderWindowBuilder::Window(window),
      webview: WebViewBuilder::new(),
    }
  }

  window_builder! {
    /// Requests the window to be of specific dimensions.
    ///
    /// See [`WindowBuilder::with_inner_size`] for details.
    method => inner_size,
    original => with_inner_size,
    arg => size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a minimum dimension size for the window.
    ///
    /// See [`WindowBuilder::with_min_inner_size`] for details.
    method => min_inner_size,
    original => with_min_inner_size,
    arg => min_size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a maximum dimension size for the window.
    ///
    /// See [`WindowBuilder::with_max_inner_size`] for details.
    method => max_inner_size,
    original => with_max_inner_size,
    arg => max_size: T,
    generic => Into<Size>
  }

  window_builder! {
    /// Sets a desired initial position for the window.
    ///
    /// See [`WindowBuilder::with_position`] for details.
    method => position,
    original => with_position,
    arg => position: T,
    generic => Into<Position>
  }

  window_builder! {
    /// Sets whether the window is resizable or not.
    ///
    /// See [`WindowBuilder::with_resizable`] for details.
    method => resizable,
    original => with_resizable,
    arg => resizable: bool,
  }

  window_builder! {
    /// Requests a specific title for the window.
    ///
    /// See [`WindowBuilder::with_title`] for details.
    method => title,
    original => with_title,
    arg => title: T,
    generic => Into<String>
  }

  window_builder! {
    /// Requests a specific menu for the window.
    ///
    /// See [`WindowBuilder::with_menu`] for details.
    method => menu,
    original => with_menu,
    arg => menu: T,
    generic => Into<Vec<Menu>>
  }

  window_builder! {
    /// Sets the window fullscreen state.
    ///
    /// See [`WindowBuilder::with_fullscreen`] for details.
    method => fullscreen,
    original => with_fullscreen,
    arg => fullscreen: Option<Fullscreen>,
  }

  window_builder! {
    /// Requests maximized mode.
    ///
    /// See [`WindowBuilder::with_maximized`] for details.
    method => maximized,
    original => with_maximized,
    arg => maximized: bool,
  }

  window_builder! {
    /// Sets whether the window will be initially hidden or visible.
    ///
    /// See [`WindowBuilder::with_visible`] for details.
    method => visible,
    original => with_visible,
    arg => visible: bool,
  }

  // todo: this is the only setter that doesn't take a bool and that seems wrong on a builder
  window_builder! {
    /// Sets whether the window will be initially hidden or focus.
    ///
    /// See [`WindowBuilder::with_focus`] for details.
    method => focus,
    original => with_focus,
  }

  window_builder! {
    /// Sets whether the background of the window should be transparent.
    ///
    /// See [`WindowBuilder::with_transparent`] for details.
    method => transparent_window,
    original => with_transparent,
    arg => transparent: bool,
  }

  window_builder! {
    /// Sets whether the window should have a border, a title bar, etc.
    ///
    /// See [`WindowBuilder::with_decorations`] for details.
    method => decorations,
    original => with_decorations,
    arg => decorations: bool,
  }

  window_builder! {
    /// Sets whether or not the window will always be on top of other windows.
    ///
    /// See [`WindowBuilder::with_always_on_top`] for details.
    method => always_on_top,
    original => with_always_on_top,
    arg => always_on_top: bool,
  }

  window_builder! {
    /// Sets the window icon.
    ///
    /// See [`WindowBuilder::with_window_icon`] for details.
    method => window_icon,
    original => with_window_icon,
    arg => window_icon: Option<Icon>,
  }

  /// Whether the [`WebView`] should be transparent.
  ///
  /// See [`WebViewBuilder::with_transparent`] for details.
  #[inline]
  pub fn transparent_webview(mut self, transparent: bool) -> Self {
    self.webview = self.webview.with_transparent(transparent);
    self
  }

  /// Set both the [`Window`] and [`WebView`] to be transparent.
  ///
  /// See [`Builder::transparent_window`] and [`Builder::transparent_webview`] for details.
  #[inline]
  pub fn transparent(self, transparent: bool) -> Self {
    self
      .transparent_window(transparent)
      .transparent_webview(transparent)
  }

  /// Initialize javascript code when loading new pages.
  ///
  /// See [`WebViewBuilder::with_initialization_script`] for details.
  #[inline]
  pub fn initialization_script(mut self, js: &str) -> Self {
    self.webview = self.webview.with_initialization_script(js);
    self
  }

  /// Create a [`Dispatcher`] to send evaluation scripts to the [`WebView`].
  ///
  /// See [`WebViewBuilder::dispatcher`] for details.
  #[inline]
  pub fn dispatcher(&self) -> Dispatcher {
    self.webview.dispatcher()
  }

  /// Register custom file loading protocol.
  ///
  /// See [`WebViewBuilder::with_custom_protocol`] for details.
  #[inline]
  pub fn custom_protocol<F>(mut self, name: String, handler: F) -> Self
  where
    F: Fn(&Window, &str) -> crate::Result<Vec<u8>> + 'static,
  {
    self.webview = self.webview.with_custom_protocol(name, handler);
    self
  }

  /// Set the RPC handler to Communicate between the host Rust code and Javascript on [`WebView`].
  ///
  /// See [`WebViewBuilder::with_rpc_handler`] for details.
  #[inline]
  pub fn rpc_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, RpcRequest) -> Option<RpcResponse> + 'static,
  {
    self.webview = self.webview.with_rpc_handler(handler);
    self
  }

  /// Set a handler closure to process incoming [`FileDropEvent`] of the [`WebView`].
  ///
  /// See [`WebViewBuilder::with_file_drop_handler`] for details.
  #[inline]
  pub fn file_drop_handler<F>(mut self, handler: F) -> Self
  where
    F: Fn(&Window, FileDropEvent) -> bool + 'static,
  {
    self.webview = self.webview.with_file_drop_handler(handler);
    self
  }

  /// The URL to initialize the [`WebView`] with.
  ///
  /// See [`WebViewBuilder::with_url`] for details.
  #[inline]
  pub fn url(mut self, url: &str) -> crate::Result<Self> {
    self.webview = self.webview.with_url(url)?;
    Ok(self)
  }

  /// Build the resulting [`WebView`].
  #[inline]
  pub fn build(self, application: &Application) -> crate::Result<WebView> {
    let window = match self.window {
      BuilderWindowBuilder::Window(window) => window,
      BuilderWindowBuilder::Builder {
        builder,
        event_loop,
      } => builder.build(event_loop)?,
    };

    self.webview.build(window, application)
  }
}

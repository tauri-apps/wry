// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::fmt;

use gtk::{prelude::*, ApplicationWindow};

use super::{
  dpi::Size,
  error::OsError,
  event_loop::EventLoopWindowTarget,
  monitor::{MonitorHandle, VideoMode},
};

pub use super::icon::{BadIcon, Icon};

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with `window.id()`.
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) u32);

impl WindowId {
  /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
  /// value of this function is that it will always be equal to itself and to future values returned
  /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
  ///
  /// **Passing this into a winit function will result in undefined behavior.**
  pub unsafe fn dummy() -> Self {
    WindowId(0)
  }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
  /// The dimensions of the window. If this is `None`, some platform-specific dimensions will be
  /// used.
  ///
  /// The default is `None`.
  pub inner_size: Option<Size>,

  /// The minimum dimensions a window can be, If this is `None`, the window will have no minimum dimensions (aside from reserved).
  ///
  /// The default is `None`.
  pub min_inner_size: Option<Size>,

  /// The maximum dimensions a window can be, If this is `None`, the maximum will have no maximum or will be set to the primary monitor's dimensions by the platform.
  ///
  /// The default is `None`.
  pub max_inner_size: Option<Size>,

  /// Whether the window is resizable or not.
  ///
  /// The default is `true`.
  pub resizable: bool,

  /// Whether the window should be set as fullscreen upon creation.
  ///
  /// The default is `None`.
  pub fullscreen: Option<Fullscreen>,

  /// The title of the window in the title bar.
  ///
  /// The default is `"winit window"`.
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

  /// The window icon.
  ///
  /// The default is `None`.
  pub window_icon: Option<Icon>,
}

impl Default for WindowAttributes {
  #[inline]
  fn default() -> WindowAttributes {
    WindowAttributes {
      inner_size: None,
      min_inner_size: None,
      max_inner_size: None,
      resizable: true,
      title: "winit window".to_owned(),
      maximized: false,
      fullscreen: None,
      visible: true,
      transparent: false,
      decorations: true,
      always_on_top: false,
      window_icon: None,
    }
  }
}

/// Object that allows you to build windows.
#[derive(Clone, Default)]
pub struct WindowBuilder {
  /// The attributes to use to create the window.
  pub window: WindowAttributes,
}

impl fmt::Debug for WindowBuilder {
  fn fmt(&self, fmtr: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmtr
      .debug_struct("WindowBuilder")
      .field("window", &self.window)
      .finish()
  }
}

impl WindowBuilder {
  /// Initializes a new `WindowBuilder` with default values.
  #[inline]
  pub fn new() -> Self {
    Default::default()
  }

  /// Requests the window to be of specific dimensions.
  ///
  /// See [`Window::set_inner_size`] for details.
  ///
  /// [`Window::set_inner_size`]: crate::window::Window::set_inner_size
  #[inline]
  pub fn with_inner_size<S: Into<Size>>(mut self, size: S) -> Self {
    self.window.inner_size = Some(size.into());
    self
  }

  /// Sets a minimum dimension size for the window.
  ///
  /// See [`Window::set_min_inner_size`] for details.
  ///
  /// [`Window::set_min_inner_size`]: crate::window::Window::set_min_inner_size
  #[inline]
  pub fn with_min_inner_size<S: Into<Size>>(mut self, min_size: S) -> Self {
    self.window.min_inner_size = Some(min_size.into());
    self
  }

  /// Sets a maximum dimension size for the window.
  ///
  /// See [`Window::set_max_inner_size`] for details.
  ///
  /// [`Window::set_max_inner_size`]: crate::window::Window::set_max_inner_size
  #[inline]
  pub fn with_max_inner_size<S: Into<Size>>(mut self, max_size: S) -> Self {
    self.window.max_inner_size = Some(max_size.into());
    self
  }

  /// Sets whether the window is resizable or not.
  ///
  /// See [`Window::set_resizable`] for details.
  ///
  /// [`Window::set_resizable`]: crate::window::Window::set_resizable
  #[inline]
  pub fn with_resizable(mut self, resizable: bool) -> Self {
    self.window.resizable = resizable;
    self
  }

  /// Requests a specific title for the window.
  ///
  /// See [`Window::set_title`] for details.
  ///
  /// [`Window::set_title`]: crate::window::Window::set_title
  #[inline]
  pub fn with_title<T: Into<String>>(mut self, title: T) -> Self {
    self.window.title = title.into();
    self
  }

  /// Sets the window fullscreen state.
  ///
  /// See [`Window::set_fullscreen`] for details.
  ///
  /// [`Window::set_fullscreen`]: crate::window::Window::set_fullscreen
  #[inline]
  pub fn with_fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {
    self.window.fullscreen = fullscreen;
    self
  }

  /// Requests maximized mode.
  ///
  /// See [`Window::set_maximized`] for details.
  ///
  /// [`Window::set_maximized`]: crate::window::Window::set_maximized
  #[inline]
  pub fn with_maximized(mut self, maximized: bool) -> Self {
    self.window.maximized = maximized;
    self
  }

  /// Sets whether the window will be initially hidden or visible.
  ///
  /// See [`Window::set_visible`] for details.
  ///
  /// [`Window::set_visible`]: crate::window::Window::set_visible
  #[inline]
  pub fn with_visible(mut self, visible: bool) -> Self {
    self.window.visible = visible;
    self
  }

  /// Sets whether the background of the window should be transparent.
  #[inline]
  pub fn with_transparent(mut self, transparent: bool) -> Self {
    self.window.transparent = transparent;
    self
  }

  /// Sets whether the window should have a border, a title bar, etc.
  ///
  /// See [`Window::set_decorations`] for details.
  ///
  /// [`Window::set_decorations`]: crate::window::Window::set_decorations
  #[inline]
  pub fn with_decorations(mut self, decorations: bool) -> Self {
    self.window.decorations = decorations;
    self
  }

  /// Sets whether or not the window will always be on top of other windows.
  ///
  /// See [`Window::set_always_on_top`] for details.
  ///
  /// [`Window::set_always_on_top`]: crate::window::Window::set_always_on_top
  #[inline]
  pub fn with_always_on_top(mut self, always_on_top: bool) -> Self {
    self.window.always_on_top = always_on_top;
    self
  }

  /// Sets the window icon.
  ///
  /// See [`Window::set_window_icon`] for details.
  ///
  /// [`Window::set_window_icon`]: crate::window::Window::set_window_icon
  #[inline]
  pub fn with_window_icon(mut self, window_icon: Option<Icon>) -> Self {
    self.window.window_icon = window_icon;
    self
  }

  /// Builds the window.
  ///
  /// Possible causes of error include denied permission, incompatible system, and lack of memory.
  ///
  /// Platform-specific behavior:
  /// - **Web**: The window is created but not inserted into the web page automatically. Please
  /// see the web platform module for more information.
  #[inline]
  pub fn build<T: 'static>(
    self,
    window_target: &EventLoopWindowTarget<T>,
  ) -> Result<Window, OsError> {
    Window::new(window_target, self.window)
    // TODO request redraw
  }
}

pub struct Window {
  /// Window id.
  pub(crate) window_id: WindowId,
  /// Gtk application window.
  pub(crate) window: gtk::ApplicationWindow,
}

impl Window {
  pub(crate) fn new<T>(
    event_loop_window_target: &EventLoopWindowTarget<T>,
    attributes: WindowAttributes,
  ) -> Result<Self, OsError> {
    let app = &event_loop_window_target.app;
    let window = gtk::ApplicationWindow::new(app);
    let window_id = WindowId(window.get_id());
    event_loop_window_target
      .windows
      .borrow_mut()
      .insert(window_id);

    // Set Width/Height & Resizable
    let scale_factor = window.get_scale_factor();
    let (width, height) = attributes
      .inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or((800, 600));
    window.set_resizable(attributes.resizable);
    if attributes.resizable {
      window.set_default_size(width, height);
    } else {
      window.set_size_request(width, height);
    }

    // Set Min/Max Size
    let geom_mask = (if attributes.min_inner_size.is_some() {
      gdk::WindowHints::MIN_SIZE
    } else {
      gdk::WindowHints::empty()
    }) | (if attributes.max_inner_size.is_some() {
      gdk::WindowHints::MAX_SIZE
    } else {
      gdk::WindowHints::empty()
    });
    let (min_width, min_height) = attributes
      .min_inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or_default();
    let (max_width, max_height) = attributes
      .max_inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or_default();
    window.set_geometry_hints::<ApplicationWindow>(
      None,
      Some(&gdk::Geometry {
        min_width,
        min_height,
        max_width,
        max_height,
        base_width: 0,
        base_height: 0,
        width_inc: 0,
        height_inc: 0,
        min_aspect: 0f64,
        max_aspect: 0f64,
        win_gravity: gdk::Gravity::Center,
      }),
      geom_mask,
    );

    // Set Transparent
    if attributes.transparent {
      if let Some(screen) = window.get_screen() {
        if let Some(visual) = screen.get_rgba_visual() {
          window.set_visual(Some(&visual));
        }
      }

      window.connect_draw(|_, cr| {
        cr.set_source_rgba(0., 0., 0., 0.);
        cr.set_operator(cairo::Operator::Source);
        cr.paint();
        cr.set_operator(cairo::Operator::Over);
        Inhibit(false)
      });
      window.set_app_paintable(true);
    }

    // Rest attributes
    window.set_title(&attributes.title);
    if attributes.fullscreen.is_some() {
      window.fullscreen();
    }
    if attributes.maximized {
      window.maximize();
    }
    window.set_visible(attributes.visible);
    window.set_decorated(attributes.decorations);
    window.set_keep_above(attributes.always_on_top);
    if let Some(icon) = attributes.window_icon {
      window.set_icon(Some(&icon.inner));
    }

    window.show_all();

    Ok(Self { window_id, window })
  }

  pub fn id(&self) -> WindowId {
    self.window_id
  }

  //TODO other setters
}

/// Fullscreen modes.
#[derive(Clone, Debug, PartialEq)]
pub enum Fullscreen {
  Exclusive(VideoMode),

  /// Providing `None` to `Borderless` will fullscreen on the current monitor.
  Borderless(Option<MonitorHandle>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Theme {
  Light,
  Dark,
}

use std::fmt;

use super::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError},
    event_loop::EventLoopWindowTarget,
};

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with `window.id()`.
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(usize);

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

    // TODO pub fullscreen: Option<Fullscreen>,

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

    // TODO pub window_icon: Option<Icon>,
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
            //fullscreen: None,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            //window_icon: None,
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
        fmtr.debug_struct("WindowBuilder")
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

    // TODO pub fn with_fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {

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

    // TODO pub fn with_window_icon(mut self, window_icon: Option<Icon>) -> Self {

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
    ) {} // TODO -> Result<Window, OsError> {
}


//! The `EventLoop` struct and assorted supporting types, including `ControlFlow`.
//!
//! If you want to send custom events to the event loop, use [`EventLoop::create_proxy()`][create_proxy]
//! to acquire an [`EventLoopProxy`][event_loop_proxy] and call its [`send_event`][send_event] method.
//!
//! See the root-level documentation for information on how to create and use an event loop to
//! handle events.
//!
//! [create_proxy]: crate::event_loop::EventLoop::create_proxy
//! [event_loop_proxy]: crate::event_loop::EventLoopProxy
//! [send_event]: crate::event_loop::EventLoopProxy::send_event
use std::ops::Deref;
use std::error::Error;
use std::fmt;
use std::process;

use gtk::prelude::*;
use gio::prelude::*;
use winit::event::{Event, StartCause, WindowEvent};
pub use winit::event_loop::ControlFlow;

/// Target that associates windows with an `EventLoop`.
///
/// This type exists to allow you to create new windows while Winit executes
/// your callback. `EventLoop` will coerce into this type (`impl<T> Deref for
/// EventLoop<T>`), so functions that take this as a parameter can also take
/// `&EventLoop`.
pub struct EventLoopWindowTarget<T> {
    _marker: std::marker::PhantomData<T>,
    _unsafe: std::marker::PhantomData<*mut ()>, // Not Send nor Sync
}

/// Provides a way to retrieve events from the system and from the windows that were registered to
/// the events loop.
///
/// An `EventLoop` can be seen more or less as a "context". Calling `EventLoop::new()`
/// initializes everything that will be required to create windows. For example on Linux creating
/// an event loop opens a connection to the X or Wayland server.
///
/// To wake up an `EventLoop` from a another thread, see the `EventLoopProxy` docs.
///
/// Note that the `EventLoop` cannot be shared across threads (due to platform-dependant logic
/// forbidding it), as such it is neither `Send` nor `Sync`. If you need cross-thread access, the
/// `Window` created from this `EventLoop` _can_ be sent to an other thread, and the
/// `EventLoopProxy` allows you to wake up an `EventLoop` from another thread.
pub struct EventLoop<T: 'static> {
    /// Gtk application
    app: gtk::Application,
    /// Window target.
    window_target: EventLoopWindowTarget<T>,
    _unsafe: std::marker::PhantomData<*mut ()>, // Not Send nor Sync
}

impl<T> fmt::Debug for EventLoop<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoop { .. }")
    }
}

impl<T> fmt::Debug for EventLoopWindowTarget<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("EventLoopWindowTarget { .. }")
    }
}

impl EventLoop<()> {
    /// Builds a new event loop with a `()` as the user event type.
    ///
    /// ***For cross-platform compatibility, the `EventLoop` must be created on the main thread.***
    /// Attempting to create the event loop on a different thread will panic. This restriction isn't
    /// strictly necessary on all platforms, but is imposed to eliminate any nasty surprises when
    /// porting to platforms that require it. `EventLoopExt::new_any_thread` functions are exposed
    /// in the relevant `platform` module if the target platform supports creating an event loop on
    /// any thread.
    ///
    /// Usage will result in display backend initialisation, this can be controlled on linux
    /// using an environment variable `WINIT_UNIX_BACKEND`. Legal values are `x11` and `wayland`.
    /// If it is not set, winit will try to connect to a wayland connection, and if it fails will
    /// fallback on x11. If this variable is set with any other value, winit will panic.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    pub fn new() -> EventLoop<()> {
        EventLoop::<()>::with_user_event()
    }
}

impl<T: 'static> EventLoop<T> {
    /// Builds a new event loop.
    ///
    /// All caveats documented in [`EventLoop::new`] apply to this function.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS:** Can only be called on the main thread.
    pub fn with_user_event() -> EventLoop<()> {
        assert_is_main_thread("new_any_thread");
        EventLoop::new_any_thread()
    }

    pub(crate) fn new_any_thread() -> EventLoop<T> {
        EventLoop::new_gtk_any_thread().expect("ailed to initialize any backend!")
    }

    fn new_gtk_any_thread() -> Result<EventLoop<T>, Box<dyn Error>> {
        let app = gtk::Application::new(Some("Winit"), gio::ApplicationFlags::FLAGS_NONE)?;

        // Create event loop window target.
        let window_target = EventLoopWindowTarget {
            _marker: std::marker::PhantomData,
            _unsafe: std::marker::PhantomData,
        };

        // Create event loop itself.
        let event_loop = Self {
            app,
            window_target,
            _unsafe: std::marker::PhantomData,
        };

        Ok(event_loop)
    }

    /// Hijacks the calling thread and initializes the winit event loop with the provided
    /// closure. Since the closure is `'static`, it must be a `move` closure if it needs to
    /// access any data from the calling context.
    ///
    /// See the [`ControlFlow`] docs for information on how changes to `&mut ControlFlow` impact the
    /// event loop's behavior.
    ///
    /// Any values not passed to this function will *not* be dropped.
    ///
    /// [`ControlFlow`]: crate::event_loop::ControlFlow
    #[inline]
    pub fn run<F>(mut self, callback: F) -> !
    where
        F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow) + 'static,
    {
        self.run_return(callback);
        process::exit(0)
    }

    pub(crate) fn run_return<F>(&mut self, mut callback: F)
    where
        F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        loop {
          gtk::main_iteration();
        }
    }

    /// Creates an `EventLoopProxy` that can be used to dispatch user events to the main event loop.
    pub fn create_proxy(&self) { // -> EventLoopProxy<T> {
        todo!()
    }

    #[inline]
    pub fn window_target(&self) -> &EventLoopWindowTarget<T> {
        &self.window_target
    }
}

impl<T> Deref for EventLoop<T> {
    type Target = EventLoopWindowTarget<T>;
    fn deref(&self) -> &EventLoopWindowTarget<T> {
        self.window_target()
    }
}

// TODO EventLoopWindowTarget EventLoopProxy


fn assert_is_main_thread(suggested_method: &str) {
    if !is_main_thread() {
        panic!(
            "Initializing the event loop outside of the main thread is a significant \
             cross-platform compatibility hazard. If you really, absolutely need to create an \
             EventLoop on a different thread, please use the `EventLoopExtUnix::{}` function.",
            suggested_method
        );
    }
}

#[cfg(target_os = "linux")]
fn is_main_thread() -> bool {
    use libc::{c_long, getpid, syscall, SYS_gettid};

    unsafe { syscall(SYS_gettid) == getpid() as c_long }
}

#[cfg(any(target_os = "dragonfly", target_os = "freebsd", target_os = "openbsd"))]
fn is_main_thread() -> bool {
    use libc::pthread_main_np;

    unsafe { pthread_main_np() == 1 }
}

#[cfg(target_os = "netbsd")]
fn is_main_thread() -> bool {
    std::thread::current().name() == Some("main")
}

use std::{
  cell::RefCell,
  collections::HashSet,
  error::Error,
  fmt, mem,
  ops::Deref,
  process,
  rc::Rc,
  rc::Weak,
  sync::mpsc::{channel, Receiver, Sender},
};

use super::EventLoopProxy;
use crate::application::event::{Event, WindowEvent};
use crate::application::window::{AppWindow, WindowId, WindowRequest};
use cacao::macos::App as CacaoApp;
pub use winit::event_loop::{ControlFlow, EventLoopClosed};

pub struct EventLoopWindowTarget<T, A> {
  /// Gtk application
  pub(crate) app: CacaoApp<A>,
  /// Window Ids of the application
  pub(crate) windows: Rc<RefCell<HashSet<WindowId>>>,
  /// Window requests sender
  pub(crate) window_requests_tx: Sender<(WindowId, WindowRequest)>,
  /// Window requests receiver
  pub(crate) window_requests_rx: Receiver<(WindowId, WindowRequest)>,
  _marker: std::marker::PhantomData<T>,
  _unsafe: std::marker::PhantomData<*mut ()>, // Not Send nor Sync
}

pub struct EventLoop<T: 'static> {
  /// Window target.
  window_target: EventLoopWindowTarget<T, AppWindow<T>>,
  /// User event sender for EventLoopProxy
  user_event_tx: Sender<T>,
  /// User event receiver
  user_event_rx: Receiver<T>,
  _unsafe: std::marker::PhantomData<*mut ()>, // Not Send nor Sync
}

impl<T> fmt::Debug for EventLoop<T> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.pad("EventLoop { .. }")
  }
}

impl<T> fmt::Debug for EventLoopWindowTarget<T, ()> {
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
  pub fn with_user_event() -> EventLoop<T> {
    EventLoop::new_cacao_thread().expect("Unable to launch cacao thread")
  }

  fn new_cacao_thread() -> Result<EventLoop<T>, Box<dyn Error>> {
    let app = CacaoApp::new("org.tauri.wry", AppWindow::new());

    // Create event loop window target.
    let (window_requests_tx, window_requests_rx) = channel();
    let window_target = EventLoopWindowTarget {
      app,
      windows: Rc::new(RefCell::new(HashSet::new())),
      window_requests_tx,
      window_requests_rx,
      _marker: std::marker::PhantomData,
      _unsafe: std::marker::PhantomData,
    };

    // Create user event channel
    let (user_event_tx, user_event_rx) = channel();

    // Create event loop itself.
    let event_loop = Self {
      window_target,
      user_event_tx,
      user_event_rx,
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
  pub fn run<F>(self, callback: F) -> !
  where
    F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T, AppWindow<T>>, &mut ControlFlow) + 'static,
  {
    self.run_return(callback);
    process::exit(0)
  }

  pub(crate) fn run_return<F>(self, mut callback: F)
  where
    F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T, AppWindow<T>>, &mut ControlFlow) + 'static,
  {
    let mut window_target = self.window_target;
    let (event_tx, event_rx) = channel::<Event<'_, T>>();

    let user_event_rx = self.user_event_rx;

    // set our callback
    let callback = unsafe {
      mem::transmute::<
        Rc<
          RefCell<
            dyn FnMut(Event<'_, T>, &EventLoopWindowTarget<T, AppWindow<T>>, &mut ControlFlow),
          >,
        >,
        Rc<
          RefCell<
            dyn FnMut(Event<'_, T>, &EventLoopWindowTarget<T, AppWindow<T>>, &mut ControlFlow),
          >,
        >,
      >(Rc::new(RefCell::new(callback)))
    };

    let weak_cb: Weak<_> = Rc::downgrade(&callback);
    //mem::drop(callback);

    window_target.app.delegate.set_event_loop_callback(weak_cb);

    window_target.app.run();
  }

  #[inline]
  pub fn window_target(&self) -> &EventLoopWindowTarget<T, AppWindow<T>> {
    &self.window_target
  }

  /// Creates an `EventLoopProxy` that can be used to dispatch user events to the main event loop.
  pub fn create_proxy(&self) -> EventLoopProxy<T> {
    EventLoopProxy {
      user_event_tx: self.user_event_tx.clone(),
    }
  }
}

impl<T> Deref for EventLoop<T> {
  type Target = EventLoopWindowTarget<T, AppWindow<T>>;
  fn deref(&self) -> &EventLoopWindowTarget<T, AppWindow<T>> {
    self.window_target()
  }
}

/// Dispatch a message on a background thread.
pub fn dispatch<T>(event: Event<'static, T>) {
  CacaoApp::<AppWindow<T>, Event<T>>::dispatch_main(event);
}

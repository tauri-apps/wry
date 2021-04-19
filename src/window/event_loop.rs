// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

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
use std::{
  cell::RefCell,
  collections::HashSet,
  error::Error,
  fmt,
  ops::Deref,
  process,
  rc::Rc,
  sync::mpsc::{channel, Receiver, SendError, Sender},
};

use gio::{prelude::*, Cancellable};
use glib::{source::idle_add_local, Continue, MainContext};
use gtk::{prelude::*, Inhibit};
pub use winit::event_loop::{ControlFlow, EventLoopClosed};

use super::{
  event::{Event, StartCause, WindowEvent},
  window::WindowId,
};

/// Target that associates windows with an `EventLoop`.
///
/// This type exists to allow you to create new windows while Winit executes
/// your callback. `EventLoop` will coerce into this type (`impl<T> Deref for
/// EventLoop<T>`), so functions that take this as a parameter can also take
/// `&EventLoop`.
pub struct EventLoopWindowTarget<T> {
  /// Gtk application
  pub(crate) app: gtk::Application,
  /// Window Ids of the application
  pub(crate) windows: Rc<RefCell<HashSet<WindowId>>>,
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
  /// Window target.
  window_target: EventLoopWindowTarget<T>,
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
    EventLoop::new_gtk_any_thread().expect("Failed to initialize any backend!")
  }

  fn new_gtk_any_thread() -> Result<EventLoop<T>, Box<dyn Error>> {
    let app = gtk::Application::new(Some("org.tauri.wry"), gio::ApplicationFlags::empty())?;
    let cancellable: Option<&Cancellable> = None;
    app.register(cancellable)?;

    // Create event loop window target.
    let window_target = EventLoopWindowTarget {
      app,
      windows: Rc::new(RefCell::new(HashSet::new())),
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
    F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow) + 'static,
  {
    self.run_return(callback);
    process::exit(0)
  }

  pub(crate) fn run_return<F>(self, mut callback: F)
  where
    F: FnMut(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow) + 'static,
  {
    let mut control_flow = ControlFlow::default();
    let app = &self.window_target.app;
    let (event_tx, event_rx) = channel::<Event<'_, T>>();

    // Send closed event when a window is removed
    let windows = self.window_target.windows.take();
    for id in windows {
      let windows_rc = self.window_target.windows.clone();
      let tx_clone = event_tx.clone();
      let window = app
        .get_window_by_id(id.0)
        .expect("Window not found in the application!");
      window.connect_delete_event(move |_, _| {
        windows_rc.borrow_mut().remove(&id);
        tx_clone
          .send(Event::WindowEvent {
            window_id: id,
            event: WindowEvent::CloseRequested,
          })
          .expect("Failed to send closed window event!");

        Inhibit(false)
      });
    }

    // Send StartCause::Init event
    let tx_clone = event_tx.clone();
    app.connect_activate(move |_| {
      tx_clone.send(Event::NewEvents(StartCause::Init)).unwrap();
    });
    app.activate();

    /*
    // User events
    let keep_running_ = keep_running.clone();
    let tx_clone = event_tx.clone();
    user_event_rx.attach(Some(&context), move |event| {
      if *keep_running_.borrow() {
        tx_clone.send(Event::UserEvent(event)).unwrap();
        glib::Continue(true)
      } else {
        glib::Continue(false)
      }
    });
    */
    let context = MainContext::default();
    context.push_thread_default();
    let window_target = self.window_target;
    let keep_running = Rc::new(RefCell::new(true));
    let keep_running_ = keep_running.clone();
    let user_event_rx = self.user_event_rx;
    idle_add_local(move || {
      // User event
      if let Ok(event) = user_event_rx.try_recv() {
        let _ = event_tx.send(Event::UserEvent(event));
      }

      match control_flow {
        ControlFlow::Exit => {
          keep_running_.replace(false);
          Continue(false)
        }
        // TODO better control flow handling
        _ => {
          if let Ok(event) = event_rx.try_recv() {
            callback(event, &window_target, &mut control_flow);
          } else {
            callback(Event::MainEventsCleared, &window_target, &mut control_flow);
          }
          Continue(true)
        }
      }
    });
    context.pop_thread_default();

    while *keep_running.borrow() {
      gtk::main_iteration();
    }
  }

  #[inline]
  pub fn window_target(&self) -> &EventLoopWindowTarget<T> {
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
  type Target = EventLoopWindowTarget<T>;
  fn deref(&self) -> &EventLoopWindowTarget<T> {
    self.window_target()
  }
}

/// Used to send custom events to `EventLoop`.
#[derive(Debug, Clone)]
pub struct EventLoopProxy<T: 'static> {
  user_event_tx: Sender<T>,
}

impl<T: 'static> EventLoopProxy<T> {
  /// Send an event to the `EventLoop` from which this proxy was created. This emits a
  /// `UserEvent(event)` event in the event loop, where `event` is the value passed to this
  /// function.
  ///
  /// Returns an `Err` if the associated `EventLoop` no longer exists.
  pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
    self
      .user_event_tx
      .send(event)
      .map_err(|SendError(error)| EventLoopClosed(error))
  }
}

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

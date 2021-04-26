#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::{EventLoop, EventLoopWindowTarget};
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{EventLoop, EventLoopWindowTarget};

use std::sync::mpsc::{SendError, Sender};
pub use winit::event_loop::{ControlFlow, EventLoopClosed};

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

use std::thread::{self, JoinHandle};

use crossbeam_channel::{unbounded, Sender};
use once_cell::sync::Lazy;

/// Servo static handle to work with other webview types and threads.
/// This creates its own event loop in another thread and using crossbean channel to communicate.
pub static SERVO: Lazy<Embedder> = Lazy::new(|| {
  let (tx, rx) = unbounded();
  let thread = thread::spawn(move || while let Ok(x) = rx.recv() {});

  Embedder { thread, tx }
});

/// Servo event loop implementation. See [`SERVO`] static for more information.
pub struct Embedder {
  thread: JoinHandle<()>,
  tx: Sender<u8>,
}

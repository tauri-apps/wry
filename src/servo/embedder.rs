use std::{
  rc::Rc,
  thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use once_cell::sync::Lazy;
use raw_window_handle::RawWindowHandle;
use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods},
  embedder_traits::EventLoopWaker,
  servo_url::ServoUrl,
  Servo,
};

use super::window::Window;

/// Servo event loop thread. See [`Embedder`] static for more information.
pub static SERVO: Lazy<Embedder> = Lazy::new(|| {
  let (embedder_tx, embedder_rx) = unbounded();
  let (user_tx, user_rx) = unbounded();
  let callback_tx = embedder_tx.clone();
  let thread = thread::spawn(move || {
    let mut servo = ServoThread::default();
    while let Ok(event) = embedder_rx.recv() {
      if servo.servo.is_none() {
        servo.init(event, callback_tx.clone());
      } else {
        servo.handle_event(event);
      }
    }
  });

  Embedder {
    thread,
    embedder_tx,
    user_rx,
  }
});

#[derive(Default)]
struct ServoThread {
  servo: Option<Servo<Window>>,
  //TODO windows collection with browser_id
}

impl ServoThread {
  fn init(&mut self, event: ServoEvent, callback: Sender<ServoEvent>) {
    if let ServoEvent::NewWebView(window) = event {
      let window = Rc::new(Window::new(window));
      let mut init_servo = Servo::new(
        Box::new(EmbedderWaker(callback)),
        window,
        Some(String::from("test")),
      );
      init_servo.servo.handle_events(vec![
        EmbedderEvent::NewBrowser(
          ServoUrl::parse("https://servo.org").unwrap(),
          init_servo.browser_id,
        ),
        EmbedderEvent::SelectBrowser(init_servo.browser_id),
      ]);
      init_servo.servo.setup_logging();
      self.servo.replace(init_servo.servo);
    } else {
      log::warn!("Received event while servo hasn't initialized yet: {event:?}");
    }
  }

  fn handle_event(&mut self, event: ServoEvent) {
    let servo = self.servo.as_mut().unwrap();
    match event {
      ServoEvent::NewWebView(_) => {
        log::warn!("New webview request while servo is already initialized. Servo hasn't support multiwebview yet.");
      }
      ServoEvent::Wake => {
        // TODO start handling events!
        dbg!(servo.get_events());
        servo.handle_events(vec![EmbedderEvent::Idle]);
        servo.recomposite();
        servo.present();
      }
    }
  }
}

#[derive(Debug)]

pub enum ServoEvent {
  NewWebView(RawWindowHandle),
  Wake,
}

unsafe impl Send for ServoEvent {}
unsafe impl Sync for ServoEvent {}

/// Servo embedder handle to work with other webview types and threads.
/// This creates its own event loop in another thread and using crossbean channel to communicate.
pub struct Embedder {
  thread: JoinHandle<()>,
  embedder_tx: Sender<ServoEvent>,
  user_rx: Receiver<ServoEvent>,
}

impl Embedder {
  /// The sender to send event for servo thread to handle.
  pub fn sender(&self) -> Sender<ServoEvent> {
    self.embedder_tx.clone()
  }

  /// The receiver to get event from servo thread.
  pub fn receiver(&self) -> Receiver<ServoEvent> {
    self.user_rx.clone()
  }
}

#[derive(Debug, Clone)]
pub struct EmbedderWaker(pub Sender<ServoEvent>);

impl EmbedderMethods for EmbedderWaker {
  fn create_event_loop_waker(&mut self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }
}

impl EventLoopWaker for EmbedderWaker {
  fn clone_box(&self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }

  fn wake(&self) {
    if let Err(e) = self.0.send(ServoEvent::Wake) {
      log::error!("Failed to send wake up event to servo thread: {}", e);
    }
  }
}

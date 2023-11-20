use std::{
  rc::Rc,
  thread::{self, JoinHandle},
};

use crossbeam_channel::{unbounded, Receiver, Sender};
use once_cell::sync::Lazy;
use raw_window_handle::RawWindowHandle;
use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods},
  embedder_traits::{EmbedderMsg, EventLoopWaker},
  msg::constellation_msg::TopLevelBrowsingContextId,
  servo_url::ServoUrl,
  Servo,
};

use crate::Rect;

use super::window::WebView;

/// Servo embedder thread. See [`Embedder`] static for more information.
pub static SERVO: Lazy<Embedder> = Lazy::new(|| {
  let (embedder_tx, embedder_rx) = unbounded();
  let callback_tx = embedder_tx.clone();
  let _thread = thread::spawn(move || {
    let mut servo = ServoThread::default();
    while let Ok(event) = embedder_rx.recv() {
      if servo.servo.is_none() {
        servo.init(event, callback_tx.clone());
      } else {
        servo.handle_embedder_event(event);
      }
    }
  });

  Embedder {
    _thread,
    embedder_tx,
  }
});

#[derive(Default)]
struct ServoThread {
  servo: Option<Servo<WebView>>,
  webview: Option<(TopLevelBrowsingContextId, Rc<WebView>)>,
}

impl ServoThread {
  fn init(&mut self, event: ServoEvent, callback: Sender<ServoEvent>) {
    if let ServoEvent::NewWebView(webview) = event {
      let webview = Rc::new(WebView::new(webview));
      let mut init_servo = Servo::new(
        Box::new(EmbedderWaker(callback)),
        webview.clone(),
        Some(String::from(
          "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/119.0",
        )),
      );

      init_servo
        .servo
        .handle_events(vec![EmbedderEvent::NewBrowser(
          ServoUrl::parse("https://servo.org").unwrap(),
          init_servo.browser_id,
        )]);
      init_servo.servo.setup_logging();
      self.servo.replace(init_servo.servo);
      self.webview.replace((init_servo.browser_id, webview));
    } else {
      log::warn!("Servo embedder received event while servo hasn't initialized yet: {event:?}");
    }
  }

  /// Handle embedder event from embedder channel. These events are came from users and servo instance.
  fn handle_embedder_event(&mut self, event: ServoEvent) {
    log::trace!("Servo embedder is handling event: {event:?}");
    match event {
      ServoEvent::NewWebView(_) => {
        log::warn!("Servo embedder got new webview request while servo is already initialized. Servo hasn't support multiwebview yet.");
      }
      ServoEvent::ResizeWebView(rect) => {
        if let Some((_, webview)) = &self.webview {
          webview.set_bounds(rect);
          self.servo().handle_events(vec![EmbedderEvent::Resize]);
        }
      }
      ServoEvent::Wake => {
        self.handle_servo_message();
      }
    }
  }

  fn handle_servo_message(&mut self) {
    let servo = self.servo();
    let mut embedder_events = vec![];
    servo.get_events().into_iter().for_each(|(w, m)| {
      log::trace!("Servo embedder is handling servo message: {m:?} with browser id: {w:?}");
      match m {
        EmbedderMsg::BrowserCreated(w) => {
          embedder_events.push(EmbedderEvent::SelectBrowser(w));
        }
        EmbedderMsg::ReadyToPresent => {
          servo.recomposite();
          servo.present();
        }
        e => {
          log::warn!("Servo embedder hasn't supported handling this message yet: {e:?}")
        }
      }
    });
    embedder_events.push(EmbedderEvent::Idle);
    let need_resize = servo.handle_events(embedder_events);
  }

  fn servo(&mut self) -> &mut Servo<WebView> {
    self.servo.as_mut().unwrap()
  }
}

#[derive(Debug)]
pub enum ServoEvent {
  NewWebView(RawWindowHandle), //TODO url, useragent
  ResizeWebView(Rect),
  Wake,
}

unsafe impl Send for ServoEvent {}
unsafe impl Sync for ServoEvent {}

/// Servo embedder is an instance to work with other webview types and threads.
/// This creates its own event loop in another thread and using crossbean channel to communicate.
pub struct Embedder {
  _thread: JoinHandle<()>,
  embedder_tx: Sender<ServoEvent>,
}

impl Embedder {
  /// The sender to send event for servo thread to handle.
  pub fn sender(&self) -> Sender<ServoEvent> {
    self.embedder_tx.clone()
  }

  // /// The receiver to get event from servo thread.
  // pub fn receiver(&self) -> Receiver<ServoEvent> {
  //   self.user_rx.clone()
  // }
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

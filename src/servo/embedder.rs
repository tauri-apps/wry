use std::{
  rc::Rc,
  sync::Arc,
};

use raw_window_handle::RawWindowHandle;
use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods},
  embedder_traits::{EmbedderMsg, EventLoopWaker},
  msg::constellation_msg::TopLevelBrowsingContextId,
  servo_url::ServoUrl,
  Servo,
};

use super::window::WebView;

pub struct Embedder {
  servo: Servo<WebView>,
  webview: (TopLevelBrowsingContextId, Rc<WebView>),
}

impl Embedder {
  pub fn new(rwh: RawWindowHandle, callback: Arc<dyn Fn() + Send + Sync>) -> Self {
    let webview = Rc::new(WebView::new(rwh));
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
    Self {
      servo: init_servo.servo,
      webview: (init_servo.browser_id, webview),
    }
  }

  // TODO handle event
  // /// Handle embedder event from embedder channel. These events are came from users and servo instance.
  // fn handle_embedder_event(&mut self, event: ServoEvent) {
  //   log::trace!("Servo embedder is handling event: {event:?}");
  //   match event {
  //     ServoEvent::ResizeWebView(rect) => {
  //       if let Some((_, webview)) = &self.webview {
  //         webview.set_bounds(rect);
  //         self.servo().handle_events(vec![EmbedderEvent::Resize]);
  //       }
  //     }
  //     ServoEvent::Wake => {
  //       self.handle_servo_message();
  //     }
  //   }
  // }

  fn handle_servo_message(&mut self) {
    let mut embedder_events = vec![];
    self.servo.get_events().into_iter().for_each(|(w, m)| {
      log::trace!("Servo embedder is handling servo message: {m:?} with browser id: {w:?}");
      match m {
        EmbedderMsg::BrowserCreated(w) => {
          embedder_events.push(EmbedderEvent::SelectBrowser(w));
        }
        EmbedderMsg::ReadyToPresent => {
          self.servo.recomposite();
          self.servo.present();
        }
        e => {
          log::warn!("Servo embedder hasn't supported handling this message yet: {e:?}")
        }
      }
    });
    embedder_events.push(EmbedderEvent::Idle);
    let need_resize = self.servo.handle_events(embedder_events);
  }
}

#[derive(Clone)]
pub struct EmbedderWaker(Arc<dyn Fn() + Send + Sync>);

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
        self.0()
  }
}

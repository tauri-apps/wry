use std::rc::Rc;

use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods},
  embedder_traits::{EmbedderMsg, EventLoopWaker},
  msg::constellation_msg::TopLevelBrowsingContextId,
  servo_url::ServoUrl,
  Servo,
};
use winit::event_loop::EventLoopProxy;

use crate::Rect;

use super::window::WebView;

pub struct Embedder {
  servo: Servo<WebView>,
  webview: (TopLevelBrowsingContextId, Rc<WebView>),
}

impl Embedder {
  pub fn new(webview: WebView, callback: EmbedderWaker) -> Self {
    let webview = Rc::new(webview);
    let mut init_servo = Servo::new(
      Box::new(callback),
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
    Embedder {
      servo: init_servo.servo,
      webview: (init_servo.browser_id, webview),
    }
  }

  // /// Handle embedder event from embedder channel. These events are came from users and servo instance.
  // fn handle_embedder_event(&mut self, event: ServoEvent) {
  //   log::trace!("Servo embedder is handling event: {event:?}");
  //   match event {
  //     ServoEvent::NewWebView(_) => {
  //       log::warn!("Servo embedder got new webview request while servo is already initialized. Servo hasn't support multiwebview yet.");
  //     }
  //     ServoEvent::ResizeWebView(rect) => {
  //       self.webview.1.set_bounds(rect);
  //       self.servo.handle_events(vec![EmbedderEvent::Resize]);
  //     }
  //     ServoEvent::Wake => {
  //       self.handle_servo_message();
  //     }
  //   }
  // }

  // fn handle_servo_message(&mut self) {
  //   let mut embedder_events = vec![];
  //   let mut need_present = false;
  //   self.servo.get_events().into_iter().for_each(|(w, m)| {
  //     log::trace!("Servo embedder is handling servo message: {m:?} with browser id: {w:?}");
  //     match m {
  //       EmbedderMsg::BrowserCreated(w) => {
  //         embedder_events.push(EmbedderEvent::SelectBrowser(w));
  //         embedder_events.push(EmbedderEvent::Idle);
  //       }
  //       // EmbedderMsg::ResizeTo(_) |
  //       EmbedderMsg::ReadyToPresent => {
  //         need_present = true;
  //       }
  //       e => {
  //         log::warn!("Servo embedder hasn't supported handling this message yet: {e:?}")
  //       }
  //     }
  //   });
  //   let need_resize = self.servo.handle_events(embedder_events);
  //   // if need_resize {
  //   //     servo.repaint_synchronously();
  //   //     servo.present();
  //   if need_present {
  //     self.servo.recomposite();
  //     self.servo.present();
  //   }
  // }
}

#[derive(Debug, Clone)]
pub struct EmbedderWaker(pub EventLoopProxy<()>);

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
    if let Err(e) = self.0.send_event(()) {
      log::error!("Failed to send wake up event to servo embedder: {}", e);
    }
  }
}

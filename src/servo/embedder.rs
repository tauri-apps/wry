use std::rc::Rc;

use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods},
  embedder_traits::{EmbedderMsg, EventLoopWaker},
  euclid::Size2D,
  servo_url::ServoUrl,
  Servo,
};
use winit::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy, EventLoopWindowTarget},
};

use super::window::WebView;

/// The Servo embedder to communicate with servo instance.
pub struct Embedder {
  servo: Servo<WebView>,
  // TODO TopLevelBrowsingContextId
  webview: Rc<WebView>,
  events: Vec<EmbedderEvent>,
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
      webview,
      events: vec![],
    }
  }

  pub fn set_control_flow(&self, event: &Event<()>, evl: &EventLoopWindowTarget<()>) {
    let control_flow = if !self.webview.is_animating() || *event == Event::Suspended {
      ControlFlow::Wait
    } else {
      ControlFlow::Poll
    };
    evl.set_control_flow(control_flow);
    log::trace!("Servo embedder sets control flow to: {control_flow:?}");
  }

  pub fn handle_winit_event(&mut self, event: Event<()>) {
    log::trace!("Servo embedder is creating ebedder event from: {event:?}");
    match event {
      Event::Suspended => {}
      Event::Resumed | Event::UserEvent(()) => {
        self.events.push(EmbedderEvent::Idle);
      }
      Event::WindowEvent {
        window_id: _,
        event,
      } => match event {
        WindowEvent::RedrawRequested => {
          self.servo.recomposite();
          self.servo.present();
          self.events.push(EmbedderEvent::Idle);
        }
        WindowEvent::Resized(size) => {
          let size = Size2D::new(size.width, size.height);
          let _ = self.webview.resize(size.to_i32());
          self.events.push(EmbedderEvent::Resize);
        }
        e => log::warn!("Servo embedder hasn't supported this window event yet: {e:?}"),
      },
      e => log::warn!("Servo embedder hasn't supported this event yet: {e:?}"),
    }
  }

  pub fn handle_servo_messages(&mut self) {
    let mut need_present = false;
    self.servo.get_events().into_iter().for_each(|(w, m)| {
      log::trace!("Servo embedder is handling servo message: {m:?} with browser id: {w:?}");
      match m {
        EmbedderMsg::BrowserCreated(w) => {
          self.events.push(EmbedderEvent::SelectBrowser(w));
        }
        EmbedderMsg::ReadyToPresent => {
          need_present = true;
        }
        e => {
          log::warn!("Servo embedder hasn't supported handling this message yet: {e:?}")
        }
      }
    });

    log::trace!(
      "Servo embedder is handling embedder events: {:?}",
      self.events
    );
    if self.servo.handle_events(self.events.drain(..)) {
      self.servo.repaint_synchronously();
      self.servo.present();
    } else if need_present {
      self.webview.request_redraw();
    }
  }
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
      log::error!(
        "Servo waker failed to send wake up event to servo embedder: {}",
        e
      );
    }
  }
}

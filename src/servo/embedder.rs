use std::rc::Rc;

use servo::{
  compositing::windowing::{EmbedderEvent, EmbedderMethods, MouseWindowEvent},
  embedder_traits::{Cursor, EmbedderMsg, EventLoopWaker},
  euclid::{Point2D, Size2D},
  script_traits::{TouchEventType, WheelDelta, WheelMode},
  servo_url::ServoUrl,
  webrender_api::{
    units::{DeviceIntPoint, DevicePoint, LayoutVector2D},
    ScrollLocation,
  },
  BrowserId, Servo,
};
use tao::{
  dpi::PhysicalPosition,
  event::{ElementState, Event, TouchPhase, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy},
  window::CursorIcon,
};

use super::window::WebView;

/// The Servo embedder to communicate with servo instance.
pub struct Embedder {
  servo: Option<Servo<WebView>>,
  browser_id: Option<BrowserId>,
  webview: Rc<WebView>,
  events: Vec<EmbedderEvent>,
  mouse_position: PhysicalPosition<f64>,
  is_shutdown: bool,
}

impl Embedder {
  pub fn new<T: 'static + Clone + Send>(webview: WebView, callback: EmbedderWaker<T>) -> Self {
    let webview = Rc::new(webview);
    let mut init_servo = Servo::new(
      Box::new(callback),
      webview.clone(),
      Some(String::from(
        "Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/119.0",
      )),
      servo::compositing::CompositeTarget::Window,
    );

    init_servo
      .servo
      .handle_events(vec![EmbedderEvent::NewBrowser(
        ServoUrl::parse("https://servo.org").unwrap(),
        init_servo.browser_id,
      )]);
    init_servo.servo.setup_logging();
    Embedder {
      servo: Some(init_servo.servo),
      webview,
      events: vec![],
      mouse_position: PhysicalPosition::default(),
      is_shutdown: false,
      browser_id: None,
    }
  }

  pub fn get_control_flow(&self, event: &Event<()>) -> ControlFlow {
    if !self.webview.is_animating() || *event == Event::Suspended {
      ControlFlow::Wait
    } else {
      ControlFlow::Poll
    }
  }

  pub fn handle_tao_event(&mut self, event: Event<()>) {
    log::trace!("Servo embedder is creating ebedder event from: {event:?}");
    match event {
      Event::Suspended => {}
      Event::Resumed | Event::UserEvent(()) => {
        self.events.push(EmbedderEvent::Idle);
      }
      Event::WindowEvent {
        window_id: _,
        event,
        ..
      } => match event {
        WindowEvent::Resized(size) => {
          let size = Size2D::new(size.width, size.height);
          self.webview.resize(size.to_i32());
          self.events.push(EmbedderEvent::Resize);
        }
        WindowEvent::CursorMoved { position, .. } => {
          let event: DevicePoint = DevicePoint::new(position.x as f32, position.y as f32);
          self.mouse_position = position;
          self
            .events
            .push(EmbedderEvent::MouseWindowMoveEventClass(event));
        }
        WindowEvent::MouseInput { state, button, .. } => {
          let button: servo::script_traits::MouseButton = match button {
            tao::event::MouseButton::Left => servo::script_traits::MouseButton::Left,
            tao::event::MouseButton::Right => servo::script_traits::MouseButton::Right,
            tao::event::MouseButton::Middle => servo::script_traits::MouseButton::Middle,
            _ => {
              log::warn!("Servo embedder hasn't supported this mouse button yet: {button:?}");
              return;
            }
          };
          let position = Point2D::new(self.mouse_position.x as f32, self.mouse_position.y as f32);

          let event: MouseWindowEvent = match state {
            ElementState::Pressed => MouseWindowEvent::MouseDown(button, position),
            ElementState::Released => MouseWindowEvent::MouseUp(button, position),
            _ => {
              log::warn!("Not supported mouse state: {state:?}");
              return;
            }
          };
          self
            .events
            .push(EmbedderEvent::MouseWindowEventClass(event));

          // tao didn't send click event, so we send it after mouse up
          if state == ElementState::Released {
            let event: MouseWindowEvent = MouseWindowEvent::Click(button, position);
            self
              .events
              .push(EmbedderEvent::MouseWindowEventClass(event));
          }
        }
        // WindowEvent::TouchpadMagnify { delta, .. } => {
        //   self.events.push(EmbedderEvent::Zoom(1.0 + delta as f32));
        // }
        WindowEvent::MouseWheel { delta, phase, .. } => {
          // FIXME: Pixels per line, should be configurable (from browser setting?) and vary by zoom level.
          const LINE_HEIGHT: f32 = 38.0;

          let (mut x, mut y, mode) = match delta {
            tao::event::MouseScrollDelta::LineDelta(x, y) => {
              (x as f64, (y * LINE_HEIGHT) as f64, WheelMode::DeltaLine)
            }
            tao::event::MouseScrollDelta::PixelDelta(position) => {
              let position = position.to_logical::<f64>(self.webview.window.scale_factor());
              (position.x, position.y, WheelMode::DeltaPixel)
            }
            _ => {
              log::warn!("Not supported mouse scroll delta: {delta:?}");
              return;
            }
          };

          // Wheel Event
          self.events.push(EmbedderEvent::Wheel(
            WheelDelta { x, y, z: 0.0, mode },
            DevicePoint::new(self.mouse_position.x as f32, self.mouse_position.y as f32),
          ));

          // Scroll Event
          // Do one axis at a time.
          if y.abs() >= x.abs() {
            x = 0.0;
          } else {
            y = 0.0;
          }

          let phase: TouchEventType = match phase {
            TouchPhase::Started => TouchEventType::Down,
            TouchPhase::Moved => TouchEventType::Move,
            TouchPhase::Ended => TouchEventType::Up,
            TouchPhase::Cancelled => TouchEventType::Cancel,
            _ => {
              log::warn!("Not supported touch phase: {phase:?}");
              return;
            }
          };

          self.events.push(EmbedderEvent::Scroll(
            ScrollLocation::Delta(LayoutVector2D::new(x as f32, y as f32)),
            DeviceIntPoint::new(self.mouse_position.x as i32, self.mouse_position.y as i32),
            phase,
          ));
        }
        WindowEvent::CloseRequested => {
          self.events.push(EmbedderEvent::Quit);
        }
        e => log::warn!("Servo embedder hasn't supported this window event yet: {e:?}"),
      },
      Event::RedrawRequested(_window_id) => {
        let Some(servo) = self.servo.as_mut() else {
          return;
        };
        servo.recomposite();
        servo.present();
        self.events.push(EmbedderEvent::Idle);
      }
      e => log::warn!("Servo embedder hasn't supported this event yet: {e:?}"),
    }
  }

  pub fn handle_servo_messages(&mut self) {
    let Some(servo) = self.servo.as_mut() else {
      return;
    };

    let mut need_present = false;

    servo.get_events().into_iter().for_each(|(w, m)| {
      log::trace!("Servo embedder is handling servo message: {m:?} with browser id: {w:?}");
      match m {
        EmbedderMsg::BrowserCreated(w) => {
          if self.browser_id.is_none() {
            self.browser_id = Some(w);
          }
          self.events.push(EmbedderEvent::SelectBrowser(w));
        }
        EmbedderMsg::ReadyToPresent => {
          need_present = true;
        }
        EmbedderMsg::SetCursor(cursor) => {
          let tao_cursor = match cursor {
            Cursor::Default => CursorIcon::Default,
            Cursor::Pointer => CursorIcon::Hand,
            Cursor::ContextMenu => CursorIcon::ContextMenu,
            Cursor::Help => CursorIcon::Help,
            Cursor::Progress => CursorIcon::Progress,
            Cursor::Wait => CursorIcon::Wait,
            Cursor::Cell => CursorIcon::Cell,
            Cursor::Crosshair => CursorIcon::Crosshair,
            Cursor::Text => CursorIcon::Text,
            Cursor::VerticalText => CursorIcon::VerticalText,
            Cursor::Alias => CursorIcon::Alias,
            Cursor::Copy => CursorIcon::Copy,
            Cursor::Move => CursorIcon::Move,
            Cursor::NoDrop => CursorIcon::NoDrop,
            Cursor::NotAllowed => CursorIcon::NotAllowed,
            Cursor::Grab => CursorIcon::Grab,
            Cursor::Grabbing => CursorIcon::Grabbing,
            Cursor::EResize => CursorIcon::EResize,
            Cursor::NResize => CursorIcon::NResize,
            Cursor::NeResize => CursorIcon::NeResize,
            Cursor::NwResize => CursorIcon::NwResize,
            Cursor::SResize => CursorIcon::SResize,
            Cursor::SeResize => CursorIcon::SeResize,
            Cursor::SwResize => CursorIcon::SwResize,
            Cursor::WResize => CursorIcon::WResize,
            Cursor::EwResize => CursorIcon::EwResize,
            Cursor::NsResize => CursorIcon::NsResize,
            Cursor::NeswResize => CursorIcon::NeswResize,
            Cursor::NwseResize => CursorIcon::NwseResize,
            Cursor::ColResize => CursorIcon::ColResize,
            Cursor::RowResize => CursorIcon::RowResize,
            Cursor::AllScroll => CursorIcon::AllScroll,
            Cursor::ZoomIn => CursorIcon::ZoomIn,
            Cursor::ZoomOut => CursorIcon::ZoomOut,
            _ => CursorIcon::Default,
          };
          self.webview.window.set_cursor_icon(tao_cursor);
        }
        EmbedderMsg::AllowNavigationRequest(pipeline_id, _url) => {
          if w.is_some() {
            self
              .events
              .push(EmbedderEvent::AllowNavigationResponse(pipeline_id, true));
          }
        }
        EmbedderMsg::CloseBrowser => {
          self.events.push(EmbedderEvent::Quit);
        }
        EmbedderMsg::Shutdown => {
          self.is_shutdown = true;
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
    if servo.handle_events(self.events.drain(..)) {
      servo.repaint_synchronously();
      servo.present();
    } else if need_present {
      self.webview.request_redraw();
    }
  }

  pub fn is_shutdown(&self) -> bool {
    self.is_shutdown
  }

  pub fn servo_client(&mut self) -> &mut Option<Servo<WebView>> {
    &mut self.servo
  }
}

#[derive(Debug, Clone)]
pub struct EmbedderWaker<T: 'static + Clone + Send>(pub EventLoopProxy<T>);

impl<T: 'static + Clone + Send> EmbedderMethods for EmbedderWaker<T> {
  fn create_event_loop_waker(&mut self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }
}

impl<T: 'static + Clone + Send> EventLoopWaker for EmbedderWaker<T> {
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

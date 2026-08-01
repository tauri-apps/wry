use std::{cell::Cell, rc::Rc};

use servo::{
  DevicePoint, EventLoopWaker, InputEvent, Preferences, RenderingContext, Servo, ServoBuilder,
  UrlRequest, WebView as ServoWebView, WebViewBuilder as ServoWebViewBuilder, WebViewDelegate,
  WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use tao::{
  event::{MouseScrollDelta, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy},
  window::Window,
};
use url::Url;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::{Error, Result};

#[derive(Clone)]
pub struct EmbedderWaker(EventLoopProxy<()>);

impl EmbedderWaker {
  pub fn new(proxy: EventLoopProxy<()>) -> Self {
    Self(proxy)
  }
}

impl EventLoopWaker for EmbedderWaker {
  fn clone_box(&self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }

  fn wake(&self) {
    if let Err(error) = self.0.send_event(()) {
      eprintln!("Servo failed to wake the Tao event loop: {error}");
    }
  }
}

struct Delegate {
  window: Rc<Window>,
  closed: Cell<bool>,
}

impl WebViewDelegate for Delegate {
  fn notify_new_frame_ready(&self, _webview: ServoWebView) {
    self.window.request_redraw();
  }

  fn notify_closed(&self, _webview: ServoWebView) {
    self.closed.set(true);
  }
}

/// Owns the current Servo instance, rendering context, and top-level webview.
pub struct Embedder {
  servo: Servo,
  webview: ServoWebView,
  rendering_context: Rc<WindowRenderingContext>,
  window: Rc<Window>,
  delegate: Rc<Delegate>,
}

impl Embedder {
  pub fn new(
    window: Window,
    proxy: EventLoopProxy<()>,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
  ) -> Result<Self> {
    let window = Rc::new(window);
    let display_handle = window.display_handle()?;
    let window_handle = window.window_handle()?;
    let rendering_context = Rc::new(
      WindowRenderingContext::new(display_handle, window_handle, window.inner_size())
        .map_err(|error| Error::Servo(format!("failed to create rendering context: {error:?}")))?,
    );
    rendering_context
      .make_current()
      .map_err(|error| Error::Servo(format!("failed to activate rendering context: {error:?}")))?;

    let mut preferences = Preferences::default();
    if let Some(background_color) = background_color {
      preferences.shell_background_color_rgba = background_color;
    }

    let servo = ServoBuilder::default()
      .preferences(preferences)
      .event_loop_waker(Box::new(EmbedderWaker::new(proxy)))
      .build();
    servo.setup_logging();

    let delegate = Rc::new(Delegate {
      window: window.clone(),
      closed: Cell::new(false),
    });
    let webview_builder =
      ServoWebViewBuilder::new(&servo, rendering_context.clone()).delegate(delegate.clone());
    let webview = match initial_headers {
      Some(headers) => {
        let webview = webview_builder.build();
        webview.load_request(UrlRequest::new(initial_url).headers(headers));
        webview
      }
      None => webview_builder.url(initial_url).build(),
    };

    servo.spin_event_loop();

    Ok(Self {
      servo,
      webview,
      rendering_context,
      window,
      delegate,
    })
  }

  pub fn handle_user_event(&self) {
    self.servo.spin_event_loop();
  }

  pub fn set_control_flow(&self, control_flow: &mut ControlFlow) {
    *control_flow = if self.webview.animating() {
      ControlFlow::Poll
    } else {
      ControlFlow::Wait
    };
  }

  pub fn handle_window_event(&self, control_flow: &mut ControlFlow, event: WindowEvent<'_>) {
    self.servo.spin_event_loop();

    match event {
      WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
      WindowEvent::Resized(size) => self.webview.resize(size),
      WindowEvent::MouseWheel { delta, .. } => {
        let (x, y, mode) = match delta {
          MouseScrollDelta::LineDelta(x, y) => {
            ((x * 76.0) as f64, (y * 76.0) as f64, WheelMode::DeltaLine)
          }
          MouseScrollDelta::PixelDelta(delta) => (delta.x, delta.y, WheelMode::DeltaPixel),
          _ => return,
        };
        self
          .webview
          .notify_input_event(InputEvent::Wheel(WheelEvent::new(
            WheelDelta { x, y, z: 0.0, mode },
            DevicePoint::default().into(),
          )));
      }
      _ => {}
    }

    self.servo.spin_event_loop();
  }

  pub fn paint(&self) {
    self.servo.spin_event_loop();
    self.webview.paint();
    self.rendering_context.present();
  }

  pub fn is_shutdown(&self) -> bool {
    self.delegate.closed.get()
  }

  pub(crate) fn set_background_color(&self, background_color: [f64; 4]) {
    self
      .servo
      .set_preference("shell_background_color_rgba", background_color.into());
    self.window.request_redraw();
    self.servo.spin_event_loop();
  }

  pub(crate) fn servo(&self) -> &Servo {
    &self.servo
  }

  pub(crate) fn webview(&self) -> &ServoWebView {
    &self.webview
  }

  pub(crate) fn window(&self) -> &Window {
    &self.window
  }
}

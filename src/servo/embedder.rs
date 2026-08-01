use std::{cell::Cell, rc::Rc, sync::Arc};

use euclid::default::{Point2D, Rect as EuclidRect, Size2D};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
  DevicePoint, EventLoopWaker, InputEvent, OffscreenRenderingContext, Preferences,
  RenderingContext, Servo, ServoBuilder, UrlRequest, WebView as ServoWebView,
  WebViewBuilder as ServoWebViewBuilder, WebViewDelegate, WheelDelta, WheelEvent, WheelMode,
  WindowRenderingContext,
};
use tao::{
  dpi::{PhysicalPosition, PhysicalSize},
  event::{MouseScrollDelta, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy},
  window::Window,
};
use url::Url;

use crate::{Error, Rect, Result};

#[derive(Clone)]
struct EmbedderWaker(Arc<dyn Fn() + Send + Sync>);

impl EmbedderWaker {
  fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
    Self(Arc::new(wake))
  }
}

impl EventLoopWaker for EmbedderWaker {
  fn clone_box(&self) -> Box<dyn EventLoopWaker> {
    Box::new(self.clone())
  }

  fn wake(&self) {
    (self.0)();
  }
}

struct Delegate {
  window: Option<Rc<Window>>,
  waker: EmbedderWaker,
  frame_ready: Cell<bool>,
  closed: Cell<bool>,
}

impl Delegate {
  fn request_repaint(&self) {
    self.frame_ready.set(true);
    if let Some(window) = &self.window {
      window.request_redraw();
    } else {
      self.waker.wake();
    }
  }
}

impl WebViewDelegate for Delegate {
  fn notify_new_frame_ready(&self, _webview: ServoWebView) {
    self.request_repaint();
  }

  fn notify_closed(&self, _webview: ServoWebView) {
    self.closed.set(true);
    self.waker.wake();
  }
}

#[derive(Clone, Copy)]
struct PhysicalBounds {
  position: PhysicalPosition<i32>,
  size: PhysicalSize<u32>,
}

impl PhysicalBounds {
  fn from_wry(bounds: Rect, scale_factor: f64) -> Self {
    let position = bounds.position.to_physical::<i32>(scale_factor);
    let mut size = bounds.size.to_physical::<u32>(scale_factor);
    size.width = size.width.max(1);
    size.height = size.height.max(1);
    Self { position, size }
  }
}

enum RenderingTarget {
  Window {
    window: Rc<Window>,
    context: Rc<WindowRenderingContext>,
  },
  Child {
    parent_context: Rc<WindowRenderingContext>,
    context: Rc<OffscreenRenderingContext>,
    bounds: Cell<Rect>,
    physical_bounds: Cell<PhysicalBounds>,
    scale_factor: Cell<f64>,
  },
}

impl RenderingTarget {
  fn rendering_context(&self) -> Rc<dyn RenderingContext> {
    match self {
      Self::Window { context, .. } => context.clone(),
      Self::Child { context, .. } => context.clone(),
    }
  }

  fn paint(&self, webview: &ServoWebView) {
    webview.paint();

    match self {
      Self::Window { context, .. } => context.present(),
      Self::Child {
        parent_context,
        context,
        physical_bounds,
        ..
      } => {
        if parent_context.make_current().is_err() {
          return;
        }
        parent_context.prepare_for_rendering();

        if let Some(render_to_parent) = context.render_to_parent_callback() {
          let bounds = physical_bounds.get();
          let parent_height = parent_context.size().height as i32;
          let width = bounds.size.width as i32;
          let height = bounds.size.height as i32;
          let target = EuclidRect::new(
            Point2D::new(
              bounds.position.x,
              parent_height - bounds.position.y - height,
            ),
            Size2D::new(width, height),
          );
          render_to_parent(parent_context.glow_gl_api().as_ref(), target);
        }
        parent_context.present();
      }
    }
  }

  fn handle_parent_resized(&self, size: PhysicalSize<u32>, webview: &ServoWebView) {
    match self {
      Self::Window { .. } => webview.resize(non_zero_size(size)),
      Self::Child { parent_context, .. } => parent_context.resize(non_zero_size(size)),
    }
  }

  fn handle_scale_factor_changed(
    &self,
    scale_factor: f64,
    parent_size: PhysicalSize<u32>,
    webview: &ServoWebView,
  ) {
    match self {
      Self::Window { .. } => webview.resize(non_zero_size(parent_size)),
      Self::Child {
        parent_context,
        context,
        bounds,
        physical_bounds,
        scale_factor: current_scale_factor,
      } => {
        current_scale_factor.set(scale_factor);
        parent_context.resize(non_zero_size(parent_size));
        let bounds = PhysicalBounds::from_wry(bounds.get(), scale_factor);
        physical_bounds.set(bounds);
        context.resize(bounds.size);
        webview.resize(bounds.size);
      }
    }
  }

  fn bounds(&self) -> Rect {
    match self {
      Self::Window { window, .. } => {
        let size = window.inner_size();
        Rect {
          position: PhysicalPosition::new(0, 0).into(),
          size: size.into(),
        }
      }
      Self::Child { bounds, .. } => bounds.get(),
    }
  }

  fn set_bounds(&self, bounds: Rect, webview: &ServoWebView) {
    match self {
      Self::Window { window, .. } => {
        let size = bounds.size.to_physical::<u32>(window.scale_factor());
        webview.resize(non_zero_size(size));
      }
      Self::Child {
        context,
        bounds: current_bounds,
        physical_bounds,
        scale_factor,
        ..
      } => {
        current_bounds.set(bounds);
        let bounds = PhysicalBounds::from_wry(bounds, scale_factor.get());
        physical_bounds.set(bounds);
        context.resize(bounds.size);
        webview.resize(bounds.size);
      }
    }
  }

  fn set_window_visible(&self, visible: bool) {
    if let Self::Window { window, .. } = self {
      window.set_visible(visible);
    }
  }

  fn focus_parent(&self) -> Result<()> {
    match self {
      Self::Window { window, .. } => {
        window.set_focus();
        Ok(())
      }
      Self::Child { .. } => Err(Error::Servo(
        "focusing the parent of an embedded Servo webview must be handled by the host runtime"
          .into(),
      )),
    }
  }
}

fn non_zero_size(mut size: PhysicalSize<u32>) -> PhysicalSize<u32> {
  size.width = size.width.max(1);
  size.height = size.height.max(1);
  size
}

/// Owns the current Servo instance, rendering context, and top-level webview.
pub struct Embedder {
  servo: Servo,
  webview: ServoWebView,
  target: RenderingTarget,
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
    let context = Rc::new(
      WindowRenderingContext::new(
        window.display_handle()?,
        window.window_handle()?,
        non_zero_size(window.inner_size()),
      )
      .map_err(|error| Error::Servo(format!("failed to create rendering context: {error:?}")))?,
    );
    let wake_proxy = proxy.clone();
    let waker = EmbedderWaker::new(move || {
      if let Err(error) = wake_proxy.send_event(()) {
        eprintln!("Servo failed to wake the Tao event loop: {error}");
      }
    });
    let delegate = Rc::new(Delegate {
      window: Some(window.clone()),
      waker: waker.clone(),
      frame_ready: Cell::new(false),
      closed: Cell::new(false),
    });
    let target = RenderingTarget::Window { window, context };
    Self::build(
      target,
      waker,
      delegate,
      initial_url,
      initial_headers,
      background_color,
    )
  }

  pub fn new_child(
    parent: &Window,
    wake: impl Fn() + Send + Sync + 'static,
    bounds: Rect,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
  ) -> Result<Self> {
    let parent_context = Rc::new(
      WindowRenderingContext::new(
        parent.display_handle()?,
        parent.window_handle()?,
        non_zero_size(parent.inner_size()),
      )
      .map_err(|error| {
        Error::Servo(format!(
          "failed to create parent rendering context: {error:?}"
        ))
      })?,
    );
    let scale_factor = parent.scale_factor();
    let physical_bounds = PhysicalBounds::from_wry(bounds, scale_factor);
    let context = Rc::new(parent_context.offscreen_context(physical_bounds.size));
    let waker = EmbedderWaker::new(wake);
    let delegate = Rc::new(Delegate {
      window: None,
      waker: waker.clone(),
      frame_ready: Cell::new(false),
      closed: Cell::new(false),
    });
    let target = RenderingTarget::Child {
      parent_context,
      context,
      bounds: Cell::new(bounds),
      physical_bounds: Cell::new(physical_bounds),
      scale_factor: Cell::new(scale_factor),
    };
    Self::build(
      target,
      waker,
      delegate,
      initial_url,
      initial_headers,
      background_color,
    )
  }

  fn build(
    target: RenderingTarget,
    waker: EmbedderWaker,
    delegate: Rc<Delegate>,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
  ) -> Result<Self> {
    target
      .rendering_context()
      .make_current()
      .map_err(|error| Error::Servo(format!("failed to activate rendering context: {error:?}")))?;

    let mut preferences = Preferences::default();
    if let Some(background_color) = background_color {
      preferences.shell_background_color_rgba = background_color;
    }

    let servo = ServoBuilder::default()
      .preferences(preferences)
      .event_loop_waker(Box::new(waker))
      .build();
    servo.setup_logging();

    let webview_builder =
      ServoWebViewBuilder::new(&servo, target.rendering_context()).delegate(delegate.clone());
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
      target,
      delegate,
    })
  }

  pub fn handle_user_event(&self) {
    self.servo.spin_event_loop();
    if matches!(self.target, RenderingTarget::Child { .. })
      && self.delegate.frame_ready.replace(false)
    {
      self.target.paint(&self.webview);
    }
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
      WindowEvent::Resized(size) => self.target.handle_parent_resized(size, &self.webview),
      WindowEvent::ScaleFactorChanged {
        scale_factor,
        new_inner_size,
      } => self
        .target
        .handle_scale_factor_changed(scale_factor, *new_inner_size, &self.webview),
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
    self.delegate.frame_ready.set(false);
    self.target.paint(&self.webview);
  }

  pub fn is_shutdown(&self) -> bool {
    self.delegate.closed.get()
  }

  pub(crate) fn set_background_color(&self, background_color: [f64; 4]) {
    self
      .servo
      .set_preference("shell_background_color_rgba", background_color.into());
    self.delegate.request_repaint();
    self.servo.spin_event_loop();
  }

  pub(crate) fn bounds(&self) -> Rect {
    self.target.bounds()
  }

  pub(crate) fn set_bounds(&self, bounds: Rect) {
    self.target.set_bounds(bounds, &self.webview);
    self.delegate.request_repaint();
    self.servo.spin_event_loop();
  }

  pub(crate) fn set_visible(&self, visible: bool) {
    self.target.set_window_visible(visible);
    if visible {
      self.webview.show();
      self.delegate.request_repaint();
    } else {
      self.webview.hide();
    }
    self.servo.spin_event_loop();
  }

  pub(crate) fn focus(&self) {
    let _ = self.target.focus_parent();
    self.webview.focus();
    self.servo.spin_event_loop();
  }

  pub(crate) fn focus_parent(&self) -> Result<()> {
    self.target.focus_parent()
  }

  pub(crate) fn servo(&self) -> &Servo {
    &self.servo
  }

  pub(crate) fn webview(&self) -> &ServoWebView {
    &self.webview
  }
}

#[cfg(test)]
mod tests {
  use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};

  use super::PhysicalBounds;
  use crate::Rect;

  #[test]
  fn converts_child_bounds_to_physical_pixels() {
    let bounds = PhysicalBounds::from_wry(
      Rect {
        position: LogicalPosition::new(10, 20).into(),
        size: LogicalSize::new(300, 200).into(),
      },
      2.0,
    );

    assert_eq!(bounds.position, PhysicalPosition::new(20, 40));
    assert_eq!(bounds.size, PhysicalSize::new(600, 400));
  }

  #[test]
  fn clamps_zero_child_dimensions_for_servo() {
    let bounds = PhysicalBounds::from_wry(
      Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: PhysicalSize::new(0, 0).into(),
      },
      1.0,
    );

    assert_eq!(bounds.size, PhysicalSize::new(1, 1));
  }
}

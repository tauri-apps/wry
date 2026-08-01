use std::{cell::Cell, collections::HashMap, rc::Rc, sync::Arc};

use euclid::{
  default::{Point2D, Rect as EuclidRect, Size2D},
  Scale,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
  protocol_handler::{
    DoneChannel, FetchContext, HttpStatus, NetworkError, ProtocolHandler, ProtocolRegistry,
    Request as ServoRequest, ResourceFetchTiming, Response as ServoResponse, ResponseBody,
  },
  DevicePoint, EventLoopWaker, InputEvent, LoadStatus, NavigationRequest,
  OffscreenRenderingContext, Preferences, RenderingContext, Servo, ServoBuilder, UrlRequest,
  UserContentManager, UserScript, WebView as ServoWebView, WebViewBuilder as ServoWebViewBuilder,
  WebViewDelegate, WheelDelta, WheelEvent, WheelMode, WindowRenderingContext,
};
use tao::{
  dpi::{PhysicalPosition, PhysicalSize},
  event::{MouseScrollDelta, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy},
  window::Window,
};
use url::Url;

use crate::{
  Error, InitializationScript, PageLoadEvent, Rect, RequestAsyncResponder, Result, WebViewId,
};

type CustomProtocolHandler =
  Box<dyn Fn(WebViewId, http::Request<Vec<u8>>, RequestAsyncResponder) + Send + Sync>;

const IPC_MESSAGE_PREFIX: &str = "__WRY_SERVO_IPC__:";
const IPC_BRIDGE_SCRIPT: &str = r#"
  Object.defineProperty(window, 'ipc', {
    value: Object.freeze({
      postMessage: function(message) {
        console.debug('__WRY_SERVO_IPC__:' + String(message));
      }
    })
  });
"#;

fn ipc_message_body(message: &str) -> Option<&str> {
  message.strip_prefix(IPC_MESSAGE_PREFIX)
}

struct CustomProtocol {
  webview_id: String,
  handler: CustomProtocolHandler,
}

impl ProtocolHandler for CustomProtocol {
  fn load<'a>(
    &'a self,
    request: &'a mut ServoRequest,
    _done_chan: &mut DoneChannel,
    _context: &FetchContext,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ServoResponse> + Send + 'a>> {
    let url = request.current_url();
    let timing_type = request.timing_type();
    let method = request.method.clone();
    let headers = request.headers.clone();
    let request = http::Request::builder()
      .method(method)
      .uri(url.as_str())
      .body(Vec::new());

    let Ok(mut request) = request else {
      return Box::pin(std::future::ready(ServoResponse::network_error(
        NetworkError::ResourceLoadError(format!("invalid custom protocol URL: {url}")),
      )));
    };
    *request.headers_mut() = headers;

    let (sender, receiver) = futures_channel::oneshot::channel();
    (self.handler)(
      &self.webview_id,
      request,
      RequestAsyncResponder {
        responder: Box::new(move |response| {
          let _ = sender.send(response);
        }),
      },
    );

    Box::pin(async move {
      match receiver.await {
        Ok(response) => {
          let (parts, body) = response.into_parts();
          let mut response = ServoResponse::new(url, ResourceFetchTiming::new(timing_type));
          response.status = HttpStatus::new_raw(
            parts.status.as_u16(),
            parts
              .status
              .canonical_reason()
              .unwrap_or_default()
              .as_bytes()
              .to_vec(),
          );
          response.headers = parts.headers;
          *response.body.lock() = ResponseBody::Done(body.into_owned());
          response
        }
        Err(_) => ServoResponse::network_error(NetworkError::ResourceLoadError(
          "custom protocol response channel closed".into(),
        )),
      }
    })
  }

  fn is_fetchable(&self) -> bool {
    true
  }

  fn is_secure(&self) -> bool {
    true
  }
}

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
  ipc_handler: Option<Box<dyn Fn(http::Request<String>)>>,
  navigation_handler: Option<Box<dyn Fn(String) -> bool>>,
  document_title_changed_handler: Option<Box<dyn Fn(String)>>,
  on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
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
  fn notify_page_title_changed(&self, _webview: ServoWebView, title: Option<String>) {
    if let Some(handler) = &self.document_title_changed_handler {
      handler(title.unwrap_or_default());
    }
  }

  fn notify_load_status_changed(&self, webview: ServoWebView, status: LoadStatus) {
    let Some(handler) = &self.on_page_load_handler else {
      return;
    };
    let event = match status {
      LoadStatus::Started => PageLoadEvent::Started,
      LoadStatus::Complete => PageLoadEvent::Finished,
      LoadStatus::HeadParsed => return,
    };
    let url = webview.url().map(|url| url.to_string()).unwrap_or_default();
    handler(event, url);
  }

  fn request_navigation(&self, _webview: ServoWebView, request: NavigationRequest) {
    if self
      .navigation_handler
      .as_ref()
      .is_some_and(|handler| !handler(request.url.to_string()))
    {
      request.deny();
    } else {
      request.allow();
    }
  }

  fn notify_new_frame_ready(&self, _webview: ServoWebView) {
    self.request_repaint();
  }

  fn notify_closed(&self, _webview: ServoWebView) {
    self.closed.set(true);
    self.waker.wake();
  }

  fn show_console_message(
    &self,
    webview: ServoWebView,
    _level: servo::ConsoleLogLevel,
    message: String,
  ) {
    let (Some(handler), Some(body)) = (self.ipc_handler.as_ref(), ipc_message_body(&message))
    else {
      return;
    };
    let uri = webview
      .url()
      .map(|url| url.to_string())
      .unwrap_or_else(|| "about:blank".into());
    let uri = uri
      .parse::<http::Uri>()
      .unwrap_or_else(|_| http::Uri::from_static("/"));
    let request = http::Request::builder()
      .uri(uri)
      .body(body.to_owned())
      .expect("the fallback IPC request URI is valid");
    handler(request);
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
  fn scale_factor(&self) -> f64 {
    match self {
      Self::Window { window, .. } => window.scale_factor(),
      Self::Child { scale_factor, .. } => scale_factor.get(),
    }
  }

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
    webview.set_hidpi_scale_factor(Scale::new(scale_factor as f32));
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
    webview_id: String,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
    initialization_scripts: Vec<InitializationScript>,
    ipc_handler: Option<Box<dyn Fn(http::Request<String>)>>,
    custom_protocols: HashMap<String, CustomProtocolHandler>,
    navigation_handler: Option<Box<dyn Fn(String) -> bool>>,
    document_title_changed_handler: Option<Box<dyn Fn(String)>>,
    on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
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
      ipc_handler,
      navigation_handler,
      document_title_changed_handler,
      on_page_load_handler,
    });
    let target = RenderingTarget::Window { window, context };
    Self::build(
      target,
      waker,
      delegate,
      webview_id,
      initial_url,
      initial_headers,
      background_color,
      initialization_scripts,
      custom_protocols,
    )
  }

  pub fn new_child(
    parent: &Window,
    wake: impl Fn() + Send + Sync + 'static,
    webview_id: String,
    bounds: Rect,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
    initialization_scripts: Vec<InitializationScript>,
    ipc_handler: Option<Box<dyn Fn(http::Request<String>)>>,
    custom_protocols: HashMap<String, CustomProtocolHandler>,
    navigation_handler: Option<Box<dyn Fn(String) -> bool>>,
    document_title_changed_handler: Option<Box<dyn Fn(String)>>,
    on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
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
      ipc_handler,
      navigation_handler,
      document_title_changed_handler,
      on_page_load_handler,
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
      webview_id,
      initial_url,
      initial_headers,
      background_color,
      initialization_scripts,
      custom_protocols,
    )
  }

  fn build(
    target: RenderingTarget,
    waker: EmbedderWaker,
    delegate: Rc<Delegate>,
    webview_id: String,
    initial_url: Url,
    initial_headers: Option<http::HeaderMap>,
    background_color: Option<[f64; 4]>,
    initialization_scripts: Vec<InitializationScript>,
    custom_protocols: HashMap<String, CustomProtocolHandler>,
  ) -> Result<Self> {
    target
      .rendering_context()
      .make_current()
      .map_err(|error| Error::Servo(format!("failed to activate rendering context: {error:?}")))?;

    let mut preferences = Preferences::default();
    if let Some(background_color) = background_color {
      preferences.shell_background_color_rgba = background_color;
    }

    let mut protocol_registry = ProtocolRegistry::default();
    for (scheme, handler) in custom_protocols {
      protocol_registry
        .register(
          &scheme,
          CustomProtocol {
            webview_id: webview_id.clone(),
            handler,
          },
        )
        .map_err(|error| {
          Error::Servo(format!(
            "failed to register custom protocol {scheme}: {error:?}"
          ))
        })?;
    }

    let servo = ServoBuilder::default()
      .preferences(preferences)
      .event_loop_waker(Box::new(waker))
      .protocol_registry(protocol_registry)
      .build();
    servo.setup_logging();

    let user_content_manager = Rc::new(UserContentManager::new(&servo));
    if delegate.ipc_handler.is_some() {
      user_content_manager.add_script(Rc::new(UserScript::from(IPC_BRIDGE_SCRIPT)));
    }
    for initialization_script in initialization_scripts {
      user_content_manager.add_script(Rc::new(UserScript::from(initialization_script.script)));
    }
    let webview_builder = ServoWebViewBuilder::new(&servo, target.rendering_context())
      .hidpi_scale_factor(Scale::new(target.scale_factor() as f32))
      .delegate(delegate.clone())
      .user_content_manager(user_content_manager);
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

  pub fn is_animating(&self) -> bool {
    self.webview.animating()
  }

  pub fn handle_window_event(&self, event: &WindowEvent<'_>) {
    self.servo.spin_event_loop();

    match event {
      WindowEvent::Resized(size) => self.target.handle_parent_resized(*size, &self.webview),
      WindowEvent::ScaleFactorChanged {
        scale_factor,
        new_inner_size,
      } => self
        .target
        .handle_scale_factor_changed(*scale_factor, **new_inner_size, &self.webview),
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

  use super::{ipc_message_body, PhysicalBounds, IPC_BRIDGE_SCRIPT, IPC_MESSAGE_PREFIX};
  use crate::Rect;

  #[test]
  fn recognizes_ipc_console_messages() {
    assert_eq!(ipc_message_body("__WRY_SERVO_IPC__:hello"), Some("hello"));
    assert_eq!(ipc_message_body("ordinary console message"), None);
  }

  #[test]
  fn ipc_bridge_uses_the_embedder_prefix() {
    assert!(IPC_BRIDGE_SCRIPT.contains(IPC_MESSAGE_PREFIX));
  }

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

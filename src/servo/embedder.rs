use std::{
  cell::{Cell, RefCell},
  collections::HashMap,
  rc::Rc,
  sync::Arc,
};

use euclid::{
  default::{Point2D, Rect as EuclidRect, Size2D},
  Scale,
};
use keyboard_types::{
  Code, CompositionEvent, CompositionState, Key, KeyState, Location, Modifiers, NamedKey,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use servo::{
  protocol_handler::{
    DoneChannel, FetchContext, HttpStatus, NetworkError, ProtocolHandler, ProtocolRegistry,
    Request as ServoRequest, ResourceFetchTiming, Response as ServoResponse, ResponseBody,
  },
  DevicePoint, EventLoopWaker, ImeEvent, InputEvent, KeyboardEvent, LoadStatus, MouseButtonAction,
  MouseButtonEvent, MouseLeftViewportEvent, MouseMoveEvent, NavigationRequest,
  OffscreenRenderingContext, Preferences, RenderingContext, Servo, ServoBuilder, TouchEvent,
  TouchEventType, TouchId, TouchPointerType, UrlRequest, UserContentManager, UserScript,
  WebView as ServoWebView, WebViewBuilder as ServoWebViewBuilder, WebViewDelegate, WheelDelta,
  WheelEvent, WheelMode, WindowRenderingContext,
};
use tao::{
  dpi::{PhysicalPosition, PhysicalSize},
  event::{ElementState, MouseButton as TaoMouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
  event_loop::{ControlFlow, EventLoopProxy},
  keyboard::{Key as TaoKey, KeyCode as TaoKeyCode, KeyLocation, ModifiersState},
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
  fn is_window(&self) -> bool {
    matches!(self, Self::Window { .. })
  }

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

  fn webview_point(&self, position: PhysicalPosition<f64>) -> Option<DevicePoint> {
    match self {
      Self::Window { window, .. } => {
        let size = window.inner_size();
        point_in_bounds(position, PhysicalPosition::new(0, 0), size)
      }
      Self::Child {
        physical_bounds, ..
      } => {
        let bounds = physical_bounds.get();
        point_in_bounds(position, bounds.position, bounds.size)
      }
    }
  }
}

fn point_in_bounds(
  point: PhysicalPosition<f64>,
  origin: PhysicalPosition<i32>,
  size: PhysicalSize<u32>,
) -> Option<DevicePoint> {
  let x = point.x - f64::from(origin.x);
  let y = point.y - f64::from(origin.y);
  (x >= 0.0 && y >= 0.0 && x < f64::from(size.width) && y < f64::from(size.height))
    .then(|| DevicePoint::new(x as f32, y as f32))
}

fn servo_key(key: &TaoKey<'_>) -> Key {
  match key {
    TaoKey::Character(character) => Key::Character((*character).to_owned()),
    TaoKey::Space => Key::Character(" ".into()),
    TaoKey::Super => Key::Named(NamedKey::Meta),
    TaoKey::Unidentified(_) | TaoKey::Dead(_) => Key::Named(NamedKey::Unidentified),
    key => Key::Named(format!("{key:?}").parse().unwrap_or(NamedKey::Unidentified)),
  }
}

fn servo_code(code: TaoKeyCode) -> Code {
  match code {
    TaoKeyCode::SuperLeft => Code::MetaLeft,
    TaoKeyCode::SuperRight => Code::MetaRight,
    TaoKeyCode::Unidentified(_) => Code::Unidentified,
    code => code.to_string().parse().unwrap_or(Code::Unidentified),
  }
}

fn servo_location(location: KeyLocation) -> Location {
  match location {
    KeyLocation::Standard => Location::Standard,
    KeyLocation::Left => Location::Left,
    KeyLocation::Right => Location::Right,
    KeyLocation::Numpad => Location::Numpad,
    _ => Location::Standard,
  }
}

fn servo_modifiers(modifiers: ModifiersState) -> Modifiers {
  let mut result = Modifiers::empty();
  result.set(Modifiers::SHIFT, modifiers.shift_key());
  result.set(Modifiers::CONTROL, modifiers.control_key());
  result.set(Modifiers::ALT, modifiers.alt_key());
  result.set(Modifiers::META, modifiers.super_key());
  result
}

fn inserted_key_text(
  key: &TaoKey<'_>,
  text: Option<&str>,
  mut modifiers: Modifiers,
) -> Option<String> {
  modifiers.remove(Modifiers::SHIFT);
  if !modifiers.is_empty() {
    return None;
  }
  if let Some(text) = text {
    return Some(text.to_owned());
  }
  match key {
    TaoKey::Character(character) => Some((*character).to_owned()),
    TaoKey::Space => Some(" ".into()),
    _ => None,
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
  cursor_position: Cell<Option<DevicePoint>>,
  modifiers: Cell<Modifiers>,
  pending_ime_text: RefCell<Option<String>>,
  pending_key_text: RefCell<Option<String>>,
  pending_key_event: Cell<bool>,
  focused: Cell<bool>,
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
      cursor_position: Cell::new(None),
      modifiers: Cell::new(Modifiers::empty()),
      pending_ime_text: RefCell::new(None),
      pending_key_text: RefCell::new(None),
      pending_key_event: Cell::new(false),
      focused: Cell::new(false),
    })
  }

  pub fn handle_user_event(&self) {
    if let Some(text) = self.pending_ime_text.borrow_mut().take() {
      self.commit_ime_text(text);
    }
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

  fn commit_ime_text(&self, text: String) {
    self
      .webview
      .notify_input_event(InputEvent::Ime(ImeEvent::Composition(CompositionEvent {
        state: CompositionState::End,
        data: text,
      })));
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
      WindowEvent::Focused(focused) => {
        if *focused && self.target.is_window() {
          self.focus_webview();
        } else if !focused {
          self.pending_ime_text.borrow_mut().take();
          self.pending_key_text.borrow_mut().take();
          self.pending_key_event.set(false);
          self.blur_webview();
        }
      }
      WindowEvent::ModifiersChanged(modifiers) => {
        self.modifiers.set(servo_modifiers(*modifiers));
      }
      WindowEvent::KeyboardInput { event, .. } if self.focused.get() => {
        let state = match event.state {
          ElementState::Pressed => KeyState::Down,
          ElementState::Released => KeyState::Up,
          _ => {
            self.servo.spin_event_loop();
            return;
          }
        };
        if state == KeyState::Down {
          let key_text = inserted_key_text(&event.logical_key, event.text, self.modifiers.get());
          if let Some(ime_text) = self.pending_ime_text.borrow_mut().take() {
            if key_text.as_deref() != Some(ime_text.as_str()) {
              self.commit_ime_text(ime_text);
            }
            self.pending_key_text.borrow_mut().take();
            self.pending_key_event.set(false);
          } else {
            *self.pending_key_text.borrow_mut() = key_text;
            self.pending_key_event.set(true);
          }
        } else {
          self.pending_key_text.borrow_mut().take();
          self.pending_key_event.set(false);
        }
        self
          .webview
          .notify_input_event(InputEvent::Keyboard(KeyboardEvent::new_without_event(
            state,
            servo_key(&event.logical_key),
            servo_code(event.physical_key),
            servo_location(event.location),
            self.modifiers.get(),
            event.repeat,
            false,
          )));
      }
      WindowEvent::ReceivedImeText(text) if self.focused.get() => {
        if self.pending_key_event.replace(false) {
          if self.pending_key_text.borrow_mut().take().as_deref() != Some(text.as_str()) {
            self.commit_ime_text(text.clone());
          }
        } else {
          if let Some(previous) = self.pending_ime_text.borrow_mut().replace(text.clone()) {
            self.commit_ime_text(previous);
          }
          self.delegate.waker.wake();
        }
      }
      WindowEvent::CursorMoved { position, .. } => {
        let previous_position = self.cursor_position.get();
        let position = self.target.webview_point(*position);
        self.cursor_position.set(position);
        match (previous_position, position) {
          (_, Some(point)) => {
            self
              .webview
              .notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(point.into())));
          }
          (Some(_), None) => {
            self
              .webview
              .notify_input_event(InputEvent::MouseLeftViewport(
                MouseLeftViewportEvent::default(),
              ));
          }
          (None, None) => {}
        }
      }
      WindowEvent::CursorLeft { .. } => {
        if self.cursor_position.replace(None).is_some() {
          self
            .webview
            .notify_input_event(InputEvent::MouseLeftViewport(
              MouseLeftViewportEvent::default(),
            ));
        }
      }
      WindowEvent::MouseInput { state, button, .. } => {
        let Some(point) = self.cursor_position.get() else {
          if *state == ElementState::Pressed {
            self.blur_webview();
          }
          self.servo.spin_event_loop();
          return;
        };
        if *state == ElementState::Pressed {
          self.focus_webview();
        }
        let action = match state {
          ElementState::Pressed => MouseButtonAction::Down,
          ElementState::Released => MouseButtonAction::Up,
          _ => {
            self.servo.spin_event_loop();
            return;
          }
        };
        let button = match button {
          TaoMouseButton::Left => servo::MouseButton::Left,
          TaoMouseButton::Right => servo::MouseButton::Right,
          TaoMouseButton::Middle => servo::MouseButton::Middle,
          TaoMouseButton::Other(button) => servo::MouseButton::Other(*button),
          _ => {
            self.servo.spin_event_loop();
            return;
          }
        };
        self
          .webview
          .notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
            action,
            button,
            point.into(),
          )));
      }
      WindowEvent::MouseWheel { delta, .. } => {
        let Some(point) = self.cursor_position.get() else {
          self.servo.spin_event_loop();
          return;
        };
        let (x, y, mode) = match delta {
          MouseScrollDelta::LineDelta(x, y) => {
            ((x * 76.0) as f64, (y * 76.0) as f64, WheelMode::DeltaPixel)
          }
          MouseScrollDelta::PixelDelta(delta) => (delta.x, delta.y, WheelMode::DeltaPixel),
          _ => {
            self.servo.spin_event_loop();
            return;
          }
        };
        self
          .webview
          .notify_input_event(InputEvent::Wheel(WheelEvent::new(
            WheelDelta { x, y, z: 0.0, mode },
            point.into(),
          )));
      }
      WindowEvent::Touch(touch) => {
        let Some(point) = self.target.webview_point(touch.location) else {
          self.servo.spin_event_loop();
          return;
        };
        let event_type = match touch.phase {
          TouchPhase::Started => TouchEventType::Down,
          TouchPhase::Moved => TouchEventType::Move,
          TouchPhase::Ended => TouchEventType::Up,
          TouchPhase::Cancelled => TouchEventType::Cancel,
          _ => {
            self.servo.spin_event_loop();
            return;
          }
        };
        if touch.phase == TouchPhase::Started {
          self.focus_webview();
        }
        self
          .webview
          .notify_input_event(InputEvent::Touch(TouchEvent::new(
            event_type,
            TouchId(touch.id as i32),
            point.into(),
            TouchPointerType::Touch,
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
    self.focus_webview();
    self.servo.spin_event_loop();
  }

  fn focus_webview(&self) {
    if !self.focused.replace(true) {
      self.webview.focus();
    }
  }

  fn blur_webview(&self) {
    if self.focused.replace(false) {
      self.webview.blur();
    }
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
  use keyboard_types::{Code, Key, Modifiers, NamedKey};
  use tao::keyboard::{Key as TaoKey, KeyCode as TaoKeyCode, ModifiersState};

  use super::{
    inserted_key_text, ipc_message_body, point_in_bounds, servo_code, servo_key, servo_modifiers,
    PhysicalBounds, IPC_BRIDGE_SCRIPT, IPC_MESSAGE_PREFIX,
  };
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

  #[test]
  fn converts_parent_coordinates_to_child_device_coordinates() {
    assert_eq!(
      point_in_bounds(
        PhysicalPosition::new(125.0, 90.0),
        PhysicalPosition::new(100, 50),
        PhysicalSize::new(200, 100),
      ),
      Some(servo::DevicePoint::new(25.0, 40.0))
    );
    assert_eq!(
      point_in_bounds(
        PhysicalPosition::new(99.0, 90.0),
        PhysicalPosition::new(100, 50),
        PhysicalSize::new(200, 100),
      ),
      None
    );
  }

  #[test]
  fn converts_tao_keyboard_values_to_dom_values() {
    assert_eq!(
      servo_key(&TaoKey::Character("x")),
      Key::Character("x".into())
    );
    assert_eq!(servo_key(&TaoKey::Enter), Key::Named(NamedKey::Enter));
    assert_eq!(servo_key(&TaoKey::Space), Key::Character(" ".into()));
    assert_eq!(servo_key(&TaoKey::Super), Key::Named(NamedKey::Meta));
    assert_eq!(servo_code(TaoKeyCode::KeyA), Code::KeyA);
    assert_eq!(servo_code(TaoKeyCode::SuperLeft), Code::MetaLeft);
  }

  #[test]
  fn converts_tao_modifier_state() {
    let modifiers = servo_modifiers(ModifiersState::SHIFT | ModifiersState::SUPER);
    assert!(modifiers.contains(Modifiers::SHIFT));
    assert!(modifiers.contains(Modifiers::META));
    assert!(!modifiers.contains(Modifiers::CONTROL));
  }

  #[test]
  fn identifies_text_already_inserted_by_a_keydown() {
    assert_eq!(
      inserted_key_text(&TaoKey::Character("A"), Some("A"), Modifiers::SHIFT),
      Some("A".into())
    );
    assert_eq!(
      inserted_key_text(&TaoKey::Character("c"), Some("c"), Modifiers::META),
      None
    );
    assert_eq!(
      inserted_key_text(&TaoKey::Process, None, Modifiers::empty()),
      None
    );
    assert_eq!(
      inserted_key_text(&TaoKey::Dead(Some('´')), Some("é"), Modifiers::empty()),
      Some("é".into())
    );
  }
}

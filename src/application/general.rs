use crate::{
  application::{App, AppProxy, InnerWebViewAttributes, InnerWindowAttributes},
  ApplicationProxy, Attributes, CustomProtocol, Error, Event as WryEvent, Icon, Message, Result,
  WebView, WebViewBuilder, WindowEvent as WryWindowEvent, WindowFileDropHandler, WindowMessage,
  WindowProxy, WindowRpcHandler,
};

#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
use windows_webview2::Windows::Win32::{Shell as shell, WindowsAndMessaging::HWND};

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, WindowBuilderExtMacOS};
pub use winit::window::WindowId;
use winit::{
  dpi::{LogicalPosition, LogicalSize},
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
  window::{Fullscreen, Icon as WinitIcon, Window, WindowAttributes, WindowBuilder},
};

use std::{
  collections::HashMap,
  sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, Mutex,
  },
};

#[cfg(target_os = "windows")]
#[cfg(feature = "winrt")]
use winit::platform::windows::WindowExtWindows;
#[cfg(target_os = "windows")]
#[cfg(feature = "win32")]
use {
  libc::c_void,
  std::ptr,
  winapi::{
    shared::windef::HWND,
    um::{
      combaseapi::{CoCreateInstance, CLSCTX_SERVER},
      shobjidl_core::{CLSID_TaskbarList, ITaskbarList},
    },
    DEFINE_GUID,
  },
  winit::platform::windows::WindowExtWindows,
};

type EventLoopProxy = winit::event_loop::EventLoopProxy<Message>;

#[derive(Clone)]
pub struct InnerApplicationProxy {
  proxy: EventLoopProxy,
  receiver: Arc<Mutex<Receiver<WryEvent>>>,
}

impl AppProxy for InnerApplicationProxy {
  fn send_message(&self, message: Message) -> Result<()> {
    self
      .proxy
      .send_event(message)
      .map_err(|_| Error::MessageSender)?;
    Ok(())
  }

  fn add_window(
    &self,
    attributes: Attributes,
    file_drop_handler: Option<WindowFileDropHandler>,
    rpc_handler: Option<WindowRpcHandler>,
    custom_protocol: Option<CustomProtocol>,
  ) -> Result<WindowId> {
    let (sender, receiver) = channel();
    self.send_message(Message::NewWindow(
      attributes,
      sender,
      file_drop_handler,
      rpc_handler,
      custom_protocol,
    ))?;
    Ok(receiver.recv()?)
  }

  fn listen_event(&self) -> Result<WryEvent> {
    let rx = self.receiver.lock().unwrap();
    Ok(rx.recv()?)
  }
}

impl From<&InnerWindowAttributes> for WindowAttributes {
  fn from(w: &InnerWindowAttributes) -> Self {
    let min_inner_size = match (w.min_width, w.min_height) {
      (Some(min_width), Some(min_height)) => Some(LogicalSize::new(min_width, min_height).into()),
      _ => None,
    };

    let max_inner_size = match (w.max_width, w.max_height) {
      (Some(max_width), Some(max_height)) => Some(LogicalSize::new(max_width, max_height).into()),
      _ => None,
    };

    let fullscreen = if w.fullscreen {
      Some(Fullscreen::Borderless(None))
    } else {
      None
    };

    Self {
      resizable: w.resizable,
      title: w.title.clone(),
      maximized: w.maximized,
      visible: w.visible,
      transparent: w.transparent,
      decorations: w.decorations,
      always_on_top: w.always_on_top,
      inner_size: Some(LogicalSize::new(w.width, w.height).into()),
      min_inner_size,
      max_inner_size,
      fullscreen,
      ..Default::default()
    }
  }
}

pub struct InnerApplication {
  webviews: HashMap<WindowId, WebView>,
  event_loop: EventLoop<Message>,
  event_loop_proxy: EventLoopProxy,
  event_channel: (Sender<WryEvent>, Arc<Mutex<Receiver<WryEvent>>>),
}

impl App for InnerApplication {
  type Id = WindowId;
  type Proxy = InnerApplicationProxy;

  fn new() -> Result<Self> {
    let event_loop = EventLoop::<Message>::with_user_event();
    let proxy = event_loop.create_proxy();
    let (tx, rx) = channel();
    Ok(Self {
      webviews: HashMap::new(),
      event_loop,
      event_loop_proxy: proxy,
      event_channel: (tx, Arc::new(Mutex::new(rx))),
    })
  }

  fn create_webview(
    &mut self,
    attributes: Attributes,
    file_drop_handler: Option<WindowFileDropHandler>,
    rpc_handler: Option<WindowRpcHandler>,
    custom_protocol: Option<CustomProtocol>,
  ) -> Result<Self::Id> {
    let (window_attrs, webview_attrs) = attributes.split();

    let window = _create_window(&self.event_loop, window_attrs)?;
    let webview = _create_webview(
      self.application_proxy(),
      window,
      custom_protocol,
      rpc_handler,
      file_drop_handler,
      webview_attrs,
    )?;

    let id = webview.window().id();
    self.webviews.insert(id, webview);
    Ok(id)
  }

  fn application_proxy(&self) -> Self::Proxy {
    InnerApplicationProxy {
      proxy: self.event_loop_proxy.clone(),
      receiver: self.event_channel.1.clone(),
    }
  }

  fn run(self) {
    let proxy = self.application_proxy();
    let mut windows = self.webviews;
    let event_sender = self.event_channel.0;

    self.event_loop.run(move |event, event_loop, control_flow| {
      *control_flow = ControlFlow::Wait;

      for (_, w) in windows.iter() {
        if let Err(e) = w.evaluate_script() {
          log::error!("{}", e);
        }
      }
      match event {
        Event::WindowEvent { event, window_id } => match event {
          WindowEvent::CloseRequested => {
            windows.remove(&window_id);
            event_sender
              .send(WryEvent::WindowEvent {
                window_id,
                event: WryWindowEvent::CloseRequested,
              })
              .unwrap();

            if windows.is_empty() {
              *control_flow = ControlFlow::Exit;
            }
          }
          WindowEvent::Resized(_) => {
            if let Err(e) = windows[&window_id].resize() {
              log::error!("{}", e);
            }
          }
          _ => {}
        },
        Event::UserEvent(message) => match message {
          Message::NewWindow(
            attributes,
            sender,
            file_drop_handler,
            rpc_handler,
            custom_protocol,
          ) => {
            let (window_attrs, webview_attrs) = attributes.split();
            match _create_window(&event_loop, window_attrs) {
              Ok(window) => {
                if let Err(e) = sender.send(window.id()) {
                  log::error!("{}", e);
                }
                match _create_webview(
                  proxy.clone(),
                  window,
                  custom_protocol,
                  rpc_handler,
                  file_drop_handler,
                  webview_attrs,
                ) {
                  Ok(webview) => {
                    let id = webview.window().id();
                    windows.insert(id, webview);
                  }
                  Err(e) => {
                    log::error!("{}", e);
                  }
                }
              }
              Err(e) => {
                log::error!("{}", e);
              }
            }
          }
          Message::Window(id, window_message) => {
            if let Some(webview) = windows.get_mut(&id) {
              let window = webview.window();
              match window_message {
                WindowMessage::SetResizable(resizable) => window.set_resizable(resizable),
                WindowMessage::SetTitle(title) => window.set_title(&title),
                WindowMessage::Maximize => window.set_maximized(true),
                WindowMessage::Unmaximize => window.set_maximized(false),
                WindowMessage::Minimize => window.set_minimized(true),
                WindowMessage::Unminimize => window.set_minimized(false),
                WindowMessage::Show => window.set_visible(true),
                WindowMessage::Hide => window.set_visible(false),
                WindowMessage::Close => {
                  windows.remove(&id);
                  if windows.is_empty() {
                    *control_flow = ControlFlow::Exit;
                  }
                }
                WindowMessage::SetDecorations(decorations) => window.set_decorations(decorations),
                WindowMessage::SetAlwaysOnTop(always_on_top) => {
                  window.set_always_on_top(always_on_top)
                }
                WindowMessage::SetWidth(width) => {
                  let mut size = window.inner_size().to_logical(window.scale_factor());
                  size.width = width;
                  window.set_inner_size(size);
                }
                WindowMessage::SetHeight(height) => {
                  let mut size = window.inner_size().to_logical(window.scale_factor());
                  size.height = height;
                  window.set_inner_size(size);
                }
                WindowMessage::Resize { width, height } => {
                  window.set_inner_size(LogicalSize::new(width, height));
                }
                WindowMessage::SetMinSize {
                  min_width,
                  min_height,
                } => {
                  window.set_min_inner_size(Some(LogicalSize::new(min_width, min_height)));
                }
                WindowMessage::SetMaxSize {
                  max_width,
                  max_height,
                } => {
                  window.set_max_inner_size(Some(LogicalSize::new(max_width, max_height)));
                }
                WindowMessage::SetX(x) => {
                  if let Ok(outer_position) = window.outer_position() {
                    let mut outer_position = outer_position.to_logical(window.scale_factor());
                    outer_position.x = x;
                    window.set_outer_position(outer_position);
                  }
                }
                WindowMessage::SetY(y) => {
                  if let Ok(outer_position) = window.outer_position() {
                    let mut outer_position = outer_position.to_logical(window.scale_factor());
                    outer_position.y = y;
                    window.set_outer_position(outer_position);
                  }
                }
                WindowMessage::SetPosition { x, y } => {
                  window.set_outer_position(LogicalPosition::new(x, y))
                }
                WindowMessage::SetFullscreen(fullscreen) => {
                  if fullscreen {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)))
                  } else {
                    window.set_fullscreen(None)
                  }
                }
                WindowMessage::SetIcon(icon) => {
                  if let Ok(icon) = load_icon(icon) {
                    window.set_window_icon(Some(icon));
                  }
                }
                WindowMessage::EvaluationScript(script) => {
                  let _ = webview.dispatch_script(&script);
                }
                WindowMessage::BeginDrag { x: _, y: _ } => {
                  window.drag_window().unwrap();
                }
              }
            }
          }
        },
        _ => (),
      }
    });
  }
}

fn load_icon(icon: Icon) -> crate::Result<WinitIcon> {
  let image = image::load_from_memory(&icon.0)?.into_rgba8();
  let (width, height) = image.dimensions();
  let rgba = image.into_raw();
  let icon = WinitIcon::from_rgba(rgba, width, height)?;
  Ok(icon)
}

#[cfg(target_os = "windows")]
fn skip_taskbar(_window: &Window) {
  #[cfg(feature = "winrt")]
  unsafe {
    if let Ok(taskbar_list) = windows::create_instance::<shell::ITaskbarList>(&shell::TaskbarList) {
      let _ = taskbar_list.DeleteTab(HWND(_window.hwnd() as _));
    }
  }
  #[cfg(feature = "win32")]
  unsafe {
    let mut taskbar_list: *mut ITaskbarList = std::mem::zeroed();
    DEFINE_GUID! {IID_ITASKBAR_LIST,
    0x56FDF342, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90}
    CoCreateInstance(
      &CLSID_TaskbarList,
      ptr::null_mut(),
      CLSCTX_SERVER,
      &IID_ITASKBAR_LIST,
      &mut taskbar_list as *mut *mut ITaskbarList as *mut *mut c_void,
    );
    (*taskbar_list).DeleteTab(_window.hwnd() as HWND);
    (*taskbar_list).Release();
  }
}

fn _create_window(
  event_loop: &EventLoopWindowTarget<Message>,
  attributes: InnerWindowAttributes,
) -> Result<Window> {
  let mut window_builder = WindowBuilder::new();
  #[cfg(target_os = "macos")]
  if attributes.skip_taskbar {
    window_builder = window_builder.with_activation_policy(ActivationPolicy::Accessory);
  }
  let window_attributes = WindowAttributes::from(&attributes);
  window_builder.window = window_attributes;
  let window = window_builder.build(event_loop)?;
  match (attributes.x, attributes.y) {
    (Some(x), Some(y)) => window.set_outer_position(LogicalPosition::new(x, y)),
    _ => {}
  }
  if let Some(icon) = attributes.icon {
    window.set_window_icon(Some(load_icon(icon)?));
  }

  #[cfg(target_os = "windows")]
  if attributes.skip_taskbar {
    skip_taskbar(&window);
  }

  Ok(window)
}

fn _create_webview(
  proxy: InnerApplicationProxy,
  window: Window,
  custom_protocol: Option<CustomProtocol>,
  rpc_handler: Option<WindowRpcHandler>,
  file_drop_handler: Option<WindowFileDropHandler>,

  attributes: InnerWebViewAttributes,
) -> Result<WebView> {
  let window_id = window.id();

  let mut webview = WebViewBuilder::new(window)?
    .transparent(attributes.transparent)
    .user_data_path(attributes.user_data_path);

  for js in attributes.initialization_scripts {
    webview = webview.initialize_script(&js);
  }

  if let Some(protocol) = custom_protocol {
    webview = webview.register_protocol(protocol.name, protocol.handler)
  }

  let proxy_ = proxy.clone();
  webview = webview.set_rpc_handler(Box::new(move |request| {
    let proxy = WindowProxy::new(
      ApplicationProxy {
        inner: proxy_.clone(),
      },
      window_id,
    );

    if &request.method == "__WRY_BEGIN_WINDOW_DRAG__" {
      if let Some(params) = &request.params {
        let x = params[0].as_f64()?;
        let y = params[1].as_f64()?;
        proxy.begin_drag(x, y).unwrap();
      }
    }

    if let Some(rpc_handler) = &rpc_handler {
      rpc_handler(proxy, request)
    } else {
      None
    }
  }));

  webview = webview.set_file_drop_handler(Some(Box::new(move |event| {
    let proxy = WindowProxy::new(
      ApplicationProxy {
        inner: proxy.clone(),
      },
      window_id,
    );

    if let Some(file_drop_handler) = &file_drop_handler {
      file_drop_handler(proxy, event)
    } else {
      false
    }
  })));

  webview = match attributes.url {
    Some(url) => webview.load_url(&url)?,
    None => webview,
  };

  let webview = webview.build()?;
  Ok(webview)
}

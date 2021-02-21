use crate::{
    application::{App, AppProxy, InnerWebViewAttributes, InnerWindowAttributes},
    ApplicationProxy, Attributes, Callback, Icon, Message, Result, WebView, WebViewBuilder,
    WindowMessage, WindowProxy,
};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, WindowBuilderExtMacOS};
pub use winit::window::WindowId;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
    window::{Fullscreen, Icon as WinitIcon, Window, WindowAttributes, WindowBuilder},
};

use std::{collections::HashMap, sync::mpsc::channel};

#[cfg(target_os = "windows")]
use {
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
}

impl AppProxy for InnerApplicationProxy {
    fn send_message(&self, message: Message) -> Result<()> {
        self.proxy.send_event(message)?;
        Ok(())
    }

    fn add_window(
        &self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<WindowId> {
        let (sender, receiver) = channel();
        self.send_message(Message::NewWindow(attributes, callbacks, sender))?;
        Ok(receiver.recv()?)
    }
}

impl From<&InnerWindowAttributes> for WindowAttributes {
    fn from(w: &InnerWindowAttributes) -> Self {
        let min_inner_size = match (w.min_width, w.min_height) {
            (Some(min_width), Some(min_height)) => {
                Some(LogicalSize::new(min_width, min_height).into())
            }
            _ => None,
        };

        let max_inner_size = match (w.max_width, w.max_height) {
            (Some(max_width), Some(max_height)) => {
                Some(LogicalSize::new(max_width, max_height).into())
            }
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
}

impl App for InnerApplication {
    type Id = WindowId;
    type Proxy = InnerApplicationProxy;

    fn new() -> Result<Self> {
        let event_loop = EventLoop::<Message>::with_user_event();
        let proxy = event_loop.create_proxy();
        Ok(Self {
            webviews: HashMap::new(),
            event_loop,
            event_loop_proxy: proxy,
        })
    }

    fn create_webview(
        &mut self,
        attributes: Attributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<Self::Id> {
        let (window_attrs, webview_attrs) = attributes.split();
        let window = _create_window(&self.event_loop, window_attrs)?;
        let webview = _create_webview(&self.application_proxy(), window, webview_attrs, callbacks)?;
        let id = webview.window().id();
        self.webviews.insert(id, webview);
        Ok(id)
    }

    fn application_proxy(&self) -> Self::Proxy {
        InnerApplicationProxy {
            proxy: self.event_loop_proxy.clone(),
        }
    }

    fn run(self) {
        let dispatcher = self.application_proxy();
        let mut windows = self.webviews;
        self.event_loop.run(move |event, event_loop, control_flow| {
            *control_flow = ControlFlow::Wait;

            for (_, w) in windows.iter() {
                w.evaluate_script().unwrap();
            }
            match event {
                Event::WindowEvent { event, window_id } => match event {
                    WindowEvent::CloseRequested => {
                        windows.remove(&window_id);

                        if windows.is_empty() {
                            *control_flow = ControlFlow::Exit;
                        }
                    }
                    WindowEvent::Resized(_) => {
                        windows[&window_id].resize().unwrap();
                    }
                    _ => {}
                },
                Event::UserEvent(message) => match message {
                    Message::NewWindow(attributes, callbacks, sender) => {
                        let (window_attrs, webview_attrs) = attributes.split();
                        let window = _create_window(&event_loop, window_attrs).unwrap();
                        sender.send(window.id()).unwrap();
                        let webview =
                            _create_webview(&dispatcher, window, webview_attrs, callbacks).unwrap();
                        let id = webview.window().id();
                        windows.insert(id, webview);
                    }
                    Message::Window(id, window_message) => {
                        if let Some(webview) = windows.get_mut(&id) {
                            let window = webview.window();
                            match window_message {
                                WindowMessage::SetResizable(resizable) => {
                                    window.set_resizable(resizable)
                                }
                                WindowMessage::SetTitle(title) => window.set_title(&title),
                                WindowMessage::Maximize => window.set_maximized(true),
                                WindowMessage::Unmaximize => window.set_maximized(false),
                                WindowMessage::Minimize => window.set_minimized(true),
                                WindowMessage::Unminimize => window.set_minimized(false),
                                WindowMessage::Show => window.set_visible(true),
                                WindowMessage::Hide => window.set_visible(false),
                                WindowMessage::SetDecorations(decorations) => {
                                    window.set_decorations(decorations)
                                }
                                WindowMessage::SetAlwaysOnTop(always_on_top) => {
                                    window.set_always_on_top(always_on_top)
                                }
                                WindowMessage::SetWidth(width) => {
                                    let mut size =
                                        window.inner_size().to_logical(window.scale_factor());
                                    size.width = width;
                                    window.set_inner_size(size);
                                }
                                WindowMessage::SetHeight(height) => {
                                    let mut size =
                                        window.inner_size().to_logical(window.scale_factor());
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
                                    window.set_min_inner_size(Some(LogicalSize::new(
                                        min_width, min_height,
                                    )));
                                }
                                WindowMessage::SetMaxSize {
                                    max_width,
                                    max_height,
                                } => {
                                    window.set_max_inner_size(Some(LogicalSize::new(
                                        max_width, max_height,
                                    )));
                                }
                                WindowMessage::SetX(x) => {
                                    if let Ok(outer_position) = window.outer_position() {
                                        let mut outer_position =
                                            outer_position.to_logical(window.scale_factor());
                                        outer_position.x = x;
                                        window.set_outer_position(outer_position);
                                    }
                                }
                                WindowMessage::SetY(y) => {
                                    if let Ok(outer_position) = window.outer_position() {
                                        let mut outer_position =
                                            outer_position.to_logical(window.scale_factor());
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
    unsafe {
        let taskbar_list: *mut ITaskbarList = std::mem::zeroed();
        DEFINE_GUID! {IID_ITASKBAR_LIST,
        0x56FDF342, 0xfd6d, 0x11d0, 0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90}
        CoCreateInstance(
            &CLSID_TaskbarList,
            ptr::null_mut(),
            CLSCTX_SERVER,
            &IID_ITASKBAR_LIST,
            &mut (taskbar_list as *mut _),
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
    dispatcher: &InnerApplicationProxy,
    window: Window,
    attributes: InnerWebViewAttributes,
    callbacks: Option<Vec<Callback>>,
) -> Result<WebView> {
    let window_id = window.id();
    let mut webview = WebViewBuilder::new(window)?
        .debug(attributes.debug)
        .transparent(attributes.transparent);
    for js in attributes.initialization_scripts {
        webview = webview.initialize_script(&js);
    }
    if let Some(cbs) = callbacks {
        for Callback { name, mut function } in cbs {
            let dispatcher = dispatcher.clone();
            webview = webview.add_callback(&name, move |_, seq, req| {
                function(
                    WindowProxy::new(
                        ApplicationProxy {
                            inner: dispatcher.clone(),
                        },
                        window_id,
                    ),
                    seq,
                    req,
                )
            });
        }
    }
    webview = match attributes.url {
        Some(url) => webview.load_url(&url)?,
        None => webview,
    };

    let webview = webview.build()?;
    Ok(webview)
}

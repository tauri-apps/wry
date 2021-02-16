use crate::{
    application::{AppWebViewAttributes, AppWindowAttributes, ApplicationExt},
    ApplicationDispatcher, Callback, Icon, Message, Result, WebView, WebViewAttributes,
    WebViewBuilder, WebviewMessage, WindowMessage,
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

#[cfg(target_os = "windows")]
use libc::c_void;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;

#[allow(dead_code)]
#[cfg(target_os = "windows")]
mod bindings {
    ::windows::include_bindings!();
}
#[cfg(target_os = "windows")]
use bindings::windows::win32::windows_and_messaging::*;

use std::{
    collections::HashMap,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex,
    },
};

type EventLoopProxy<T> = Arc<Mutex<winit::event_loop::EventLoopProxy<Message<T>>>>;

#[derive(Clone)]
pub struct AppDispatcher<T: 'static> {
    proxy: EventLoopProxy<T>,
}

impl<T> ApplicationDispatcher<T> for AppDispatcher<T> {
    fn dispatch_message(&self, message: Message<T>) -> Result<()> {
        self.proxy
            .lock()
            .unwrap()
            .send_event(message)
            .unwrap_or_else(|_| panic!("failed to dispatch message to event loop"));
        Ok(())
    }

    fn add_window(
        &self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<WindowId> {
        let (sender, receiver): (Sender<WindowId>, Receiver<WindowId>) = channel();
        self.dispatch_message(Message::NewWindow(attributes, callbacks, sender))?;
        Ok(receiver.recv().unwrap())
    }
}

impl From<&AppWindowAttributes> for WindowAttributes {
    fn from(w: &AppWindowAttributes) -> Self {
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

fn _create_window<T>(
    event_loop: &EventLoopWindowTarget<Message<T>>,
    attributes: AppWindowAttributes,
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
    window: Window,
    attributes: AppWebViewAttributes,
    callbacks: Option<Vec<Callback>>,
) -> Result<WebView> {
    let mut webview = WebViewBuilder::new(window)?;
    for js in attributes.initialization_scripts {
        webview = webview.initialize_script(&js);
    }
    if let Some(cbs) = callbacks {
        for Callback { name, function } in cbs {
            webview = webview.add_callback(&name, function);
        }
    }
    webview = match attributes.url {
        Some(url) => webview.load_url(&url)?,
        None => webview,
    };

    let webview = webview.build()?;
    Ok(webview)
}

pub struct Application<T: 'static> {
    webviews: HashMap<WindowId, WebView>,
    event_loop: EventLoop<Message<T>>,
    event_loop_proxy: EventLoopProxy<T>,
    message_handler: Option<Box<dyn FnMut(T)>>,
}

impl<T> ApplicationExt<'_, T> for Application<T> {
    type Id = WindowId;
    type Dispatcher = AppDispatcher<T>;

    fn new() -> Result<Self> {
        let event_loop = EventLoop::<Message<T>>::with_user_event();
        let proxy = event_loop.create_proxy();
        Ok(Self {
            webviews: HashMap::new(),
            event_loop,
            event_loop_proxy: Arc::new(Mutex::new(proxy)),
            message_handler: None,
        })
    }

    fn create_webview(
        &mut self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<Self::Id> {
        let (window_attrs, webview_attrs) = attributes.split();
        let window = _create_window(&self.event_loop, window_attrs)?;
        let webview = _create_webview(window, webview_attrs, callbacks)?;
        let id = webview.window().id();
        self.webviews.insert(id, webview);
        Ok(id)
    }

    fn set_message_handler<F: FnMut(T) + 'static>(&mut self, handler: F) {
        self.message_handler.replace(Box::new(handler));
    }

    fn dispatcher(&self) -> Self::Dispatcher {
        AppDispatcher {
            proxy: self.event_loop_proxy.clone(),
        }
    }

    fn run(self) {
        let mut windows = self.webviews;
        let mut message_handler = self.message_handler;
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
                        windows[&window_id].resize();
                    }
                    _ => {}
                },
                Event::UserEvent(message) => match message {
                    Message::NewWindow(attributes, callbacks, sender) => {
                        let (window_attrs, webview_attrs) = attributes.split();
                        let window = _create_window(&event_loop, window_attrs).unwrap();
                        sender.send(window.id()).unwrap();
                        let webview = _create_webview(window, webview_attrs, callbacks).unwrap();
                        let id = webview.window().id();
                        windows.insert(id, webview);
                    }
                    Message::Webview(id, webview_message) => {
                        if let Some(webview) = windows.get_mut(&id) {
                            match webview_message {
                                WebviewMessage::EvalScript(script) => {
                                    let _ = webview.dispatch_script(&script);
                                }
                            }
                        }
                    }
                    Message::Window(id, window_message) => {
                        if let Some(webview) = windows.get(&id) {
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
                                WindowMessage::SetTransparent(_transparent) => {
                                    // TODO
                                }
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
                            }
                        }
                    }
                    Message::Custom(message) => {
                        if let Some(ref mut handler) = message_handler {
                            handler(message);
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
extern "C" {
    fn SkipTaskbar(window: HWND) -> c_void;
}

#[cfg(target_os = "windows")]
fn skip_taskbar(window: &Window) {
    unsafe {
        SkipTaskbar(HWND(window.hwnd() as isize));
    }
}

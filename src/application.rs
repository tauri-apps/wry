use crate::{Result, WebView, WebViewBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowAttributes, WindowBuilder, WindowId},
};

use std::collections::HashMap;

pub struct Callback {
    pub name: String,
    pub function: Box<dyn FnMut(i32, Vec<String>) -> i32 + Send>,
    pub evaluation_script: Option<String>,
}

// TODO complete fields on WindowAttribute
/// Attributes to use when creating a webview window.
pub struct WebViewAttributes {
    /// Whether the window is resizable or not.
    ///
    /// The default is `true`.
    pub resizable: bool,

    /// The title of the window in the title bar.
    ///
    /// The default is `"wry"`.
    pub title: String,

    /// Whether the window should be maximized upon creation.
    ///
    /// The default is `false`.
    pub maximized: bool,

    /// Whether the window should be immediately visible upon creation.
    ///
    /// The default is `true`.
    pub visible: bool,

    /// Whether the the window should be transparent. If this is true, writing colors
    /// with alpha values different than `1.0` will produce a transparent window.
    ///
    /// The default is `false`.
    pub transparent: bool,

    /// Whether the window should have borders and bars.
    ///
    /// The default is `true`.
    pub decorations: bool,

    /// Whether the window should always be on top of other windows.
    ///
    /// The default is `false`.
    pub always_on_top: bool,

    pub url: Option<String>,

    pub initialization_script: Vec<String>,

    pub bind: Vec<Callback>,
}

impl Default for WebViewAttributes {
    #[inline]
    fn default() -> WebViewAttributes {
        WebViewAttributes {
            resizable: true,
            title: "wry".to_owned(),
            maximized: false,
            visible: true,
            transparent: false,
            decorations: true,
            always_on_top: false,
            url: None,
            initialization_script: Vec::default(),
            bind: Vec::default(),
        }
    }
}

impl From<&WebViewAttributes> for WindowAttributes {
    fn from(w: &WebViewAttributes) -> Self {
        Self {
            resizable: w.resizable,
            title: w.title.clone(),
            maximized: w.maximized,
            visible: w.visible,
            transparent: w.transparent,
            decorations: w.decorations,
            always_on_top: w.always_on_top,
            ..Default::default()
        }
    }
}

pub struct Application {
    webviews: HashMap<WindowId, WebView>,
    event_loop: EventLoop<()>,
}

impl Application {
    pub fn new() -> Self {
        Self {
            webviews: HashMap::new(),
            event_loop: EventLoop::new(),
        }
    }

    pub fn add_webview(&mut self, attributes: WebViewAttributes) -> Result<()> {
        let window_attributes = WindowAttributes::from(&attributes);
        let mut window = WindowBuilder::new();
        window.window = window_attributes;

        let webview = WebViewBuilder::new(window.build(&self.event_loop)?)?;
        for js in attributes.initialization_script {
            webview.init_with_self(&js)?;
        }
        let webview = match attributes.url {
            Some(url) => webview.load_url(&url)?,
            None => webview,
        };
        let dispatcher = webview.dispatch_sender();
        for f in attributes.bind {
            webview.bind_with_self(&f.name, f.function)?;
            if let Some(script) = f.evaluation_script {
                dispatcher.send(&script)?;
            }
        }
        let webview = webview.build()?;

        let id = webview.window().id();
        self.webviews.insert(id, webview);
        Ok(())
    }

    pub fn run(self) {
        let mut windows = self.webviews;
        self.event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::WindowEvent { event, window_id } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            // This drops the window, causing it to close.
                            windows.remove(&window_id);

                            if windows.is_empty() {
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                        WindowEvent::Resized(_) => {
                            windows[&window_id].resize();
                        }
                        _ => {
                            if let Some(w) = windows.get_mut(&window_id) {
                                w.evaluate().unwrap();
                            }
                        }
                    }
                }
                _ => (),
            }
        });
    }
}

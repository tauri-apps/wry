use crate::{
    AppWindowAttributes, ApplicationExt, Callback, Result, WebView, WebViewAttributes,
    WebViewBuilder, WindowExt,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowBuilder, WindowId},
};

use std::collections::HashMap;

pub struct WinitWindow(Window);

impl WindowExt<'_> for WinitWindow {
    type Id = WindowId;
    fn id(&self) -> Self::Id {
        self.0.id()
    }
}

pub struct Application {
    webviews: HashMap<WindowId, WebView>,
    event_loop: EventLoop<()>,
}

impl ApplicationExt<'_> for Application {
    type Window = WinitWindow;
    fn new() -> Result<Self> {
        Ok(Self {
            webviews: HashMap::new(),
            event_loop: EventLoop::new(),
        })
    }

    fn create_window(&self, attributes: AppWindowAttributes) -> Result<Self::Window> {
        let mut window_builder = WindowBuilder::new();
        let window_attributes = WindowAttributes::from(&attributes);
        window_builder.window = window_attributes;
        let window = window_builder.build(&self.event_loop)?;
        Ok(WinitWindow(window))
    }

    fn create_webview(
        &mut self,
        window: Self::Window,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<()> {
        let mut webview = WebViewBuilder::new(window.0)?;
        for js in attributes.initialization_script {
            webview = webview.initialize_script(&js)?;
        }
        if let Some(cbs) = callbacks {
            for Callback { name, function } in cbs {
                webview = webview.add_callback(&name, function)?;
            }
        }
        webview = match attributes.url {
            Some(url) => webview.load_url(&url)?,
            None => webview,
        };

        let webview = webview.build()?;
        let id = webview.window().id();
        self.webviews.insert(id, webview);
        Ok(())
    }

    fn run(self) {
        let mut windows = self.webviews;
        self.event_loop.run(move |event, _, control_flow| {
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
                _ => (),
            }
        });
    }
}

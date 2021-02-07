use crate::{Callback, Result, WebView, WebViewAttributes, WebViewBuilder};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{WindowAttributes, WindowBuilder, WindowId},
};

use std::collections::HashMap;

pub struct Application {
    webviews: HashMap<WindowId, WebView>,
    event_loop: EventLoop<()>,
}

impl Application {
    pub fn new() -> Result<Self> {
        Ok(Self {
            webviews: HashMap::new(),
            event_loop: EventLoop::new(),
        })
    }

    pub fn create_window(
        &mut self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<()> {
        let window_attributes = WindowAttributes::from(&attributes);
        let mut window = WindowBuilder::new();
        window.window = window_attributes;

        let window = window.build(&self.event_loop)?;
        let mut webview = WebViewBuilder::new(window)?;
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

    pub fn run(self) {
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

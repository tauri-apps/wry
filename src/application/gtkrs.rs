use crate::{Callback, Result, WebView, WebViewAttributes, WebViewBuilder};

use std::collections::HashMap;

use gio::{ApplicationExt, Cancellable};
use gtk::{Application as GtkApp, ApplicationWindow, ApplicationWindowExt};

pub struct Application {
    webviews: HashMap<u32, WebView>,
    app: GtkApp,
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = GtkApp::new(None, Default::default())?;
        let cancellable: Option<&Cancellable> = None;
        app.register(cancellable)?;
        app.activate();
        Ok(Self {
            webviews: HashMap::new(),
            app,
        })
    }

    pub fn create_window(
        &mut self,
        attributes: WebViewAttributes,
        callbacks: Option<Vec<Callback>>,
    ) -> Result<()> {
        //TODO window config
        let window = ApplicationWindow::new(&self.app);

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
        let id = webview.window().get_id();
        self.webviews.insert(id, webview);

        Ok(())
    }

    pub fn run(self) {
        loop {
            for (_, w) in self.webviews.iter() {
                let _ = w.evaluate_script();
            }
            gtk::main_iteration();
        }
    }
}

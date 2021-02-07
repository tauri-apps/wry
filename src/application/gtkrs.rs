use crate::{
    AppWindowAttributes, ApplicationExt, Callback, Result, WebView, WebViewAttributes,
    WebViewBuilder, WindowExt,
};

use std::collections::HashMap;

use gio::{ApplicationExt as GioApplicationExt, Cancellable};
use gtk::{Application as GtkApp, ApplicationWindow, ApplicationWindowExt};

pub struct Application {
    webviews: HashMap<u32, WebView>,
    app: GtkApp,
}

pub struct GtkWindow(ApplicationWindow);

impl WindowExt<'_> for GtkWindow {
    type Id = u32;
    fn id(&self) -> Self::Id {
        self.0.get_id()
    }
}

impl ApplicationExt<'_> for Application {
    type Window = GtkWindow;

    fn new() -> Result<Self> {
        let app = GtkApp::new(None, Default::default())?;
        let cancellable: Option<&Cancellable> = None;
        app.register(cancellable)?;
        app.activate();
        Ok(Self {
            webviews: HashMap::new(),
            app,
        })
    }

    fn create_window(&self, _attributes: AppWindowAttributes) -> Result<Self::Window> {
        //TODO window config
        let window = ApplicationWindow::new(&self.app);
        Ok(GtkWindow(window))
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
        let id = webview.window().get_id();
        self.webviews.insert(id, webview);

        Ok(())
    }

    fn run(self) {
        loop {
            for (_, w) in self.webviews.iter() {
                let _ = w.evaluate_script();
            }
            gtk::main_iteration();
        }
    }
}

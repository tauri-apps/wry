use crate::{Result, WebView, WebViewBuilder};

use std::collections::HashMap;

use gio::{ApplicationExt, Cancellable};
use gtk::{Application as GtkApp, ApplicationWindow, ApplicationWindowExt};

// TODO complete fields on WindowAttribute
/// Attributes to use when creating a webview window.
#[derive(Debug, Clone)]
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
        }
    }
}

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

    pub fn create_webview(&self, attributes: WebViewAttributes) -> Result<WebViewBuilder> {
        //TODO window config
        let window = ApplicationWindow::new(&self.app);

        let mut webview = WebViewBuilder::new(window)?;
        for js in attributes.initialization_script {
            webview = webview.initialize_script(&js)?;
        }
        webview = match attributes.url {
            Some(url) => webview.load_url(&url)?,
            None => webview,
        };

        Ok(webview)
    }

    pub fn add_webview(&mut self, webview: WebView) {
        let id = webview.window().get_id();
        self.webviews.insert(id, webview);
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

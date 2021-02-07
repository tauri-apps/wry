use crate::platform::{InnerWebView, CALLBACKS};
use crate::Result;

use std::sync::mpsc::{channel, Receiver, Sender};

use url::Url;

#[cfg(target_os = "linux")]
use gtk::ApplicationWindow as Window;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

const DEBUG: bool = true;

pub struct WebViewBuilder {
    inner: WebView,
    url: Option<Url>,
}

impl WebViewBuilder {
    pub fn new(window: Window) -> Result<Self> {
        Ok(Self {
            inner: WebView::new(window)?,
            url: None,
        })
    }

    pub fn initialize_script(self, js: &str) -> Result<Self> {
        self.inner.webview.init(js)?;
        Ok(self)
    }

    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.inner.tx.clone())
    }

    // TODO rename
    pub fn add_callback<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send + 'static,
    {
        let js = format!(
            r#"var name = {:?};
                var RPC = window._rpc = (window._rpc || {{nextSeq: 1}});
                window[name] = function() {{
                var seq = RPC.nextSeq++;
                var promise = new Promise(function(resolve, reject) {{
                    RPC[seq] = {{
                    resolve: resolve,
                    reject: reject,
                    }};
                }});
                window.external.invoke(JSON.stringify({{
                    id: seq,
                    method: name,
                    params: Array.prototype.slice.call(arguments),
                }}));
                return promise;
                }}
            "#,
            name
        );
        self.inner.webview.init(&js)?;
        let dispatcher = self.dispatcher();
        CALLBACKS
            .lock()
            .unwrap()
            .insert(name.to_string(), (Box::new(f), dispatcher));
        Ok(self)
    }

    pub fn load_url(mut self, url: &str) -> Result<Self> {
        self.url = Some(Url::parse(url)?);
        Ok(self)
    }

    pub fn build(self) -> Result<WebView> {
        if let Some(url) = self.url {
            if url.cannot_be_a_base() {
                self.inner.webview.navigate_to_string(url.as_str())?;
            } else {
                self.inner.webview.navigate(url.as_str())?;
            }
        }
        Ok(self.inner)
    }
}

pub struct WebView {
    window: Window,
    webview: InnerWebView,
    tx: Sender<String>,
    rx: Receiver<String>,
}

impl WebView {
    pub fn new(window: Window) -> Result<Self> {
        #[cfg(target_os = "windows")]
        let webview = InnerWebView::new(window.hwnd())?;
        #[cfg(target_os = "macos")]
        let webview = InnerWebView::new(&window, DEBUG)?;
        #[cfg(target_os = "linux")]
        let webview = InnerWebView::new(&window, DEBUG);
        let (tx, rx) = channel();
        Ok(Self {
            window,
            webview,
            tx,
            rx,
        })
    }

    pub fn dispatch_script(&mut self, js: &str) -> Result<()> {
        self.tx.send(js.to_string())?;
        Ok(())
    }

    pub fn dispatcher(&self) -> Dispatcher {
        Dispatcher(self.tx.clone())
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn evaluate_script(&self) -> Result<()> {
        while let Ok(js) = self.rx.try_recv() {
            self.webview.eval(&js)?;
        }

        Ok(())
    }

    pub fn resize(&self) {
        #[cfg(target_os = "windows")]
        self.webview.resize(self.window.hwnd());
    }
}

#[derive(Clone)]
pub struct Dispatcher(Sender<String>);

impl Dispatcher {
    pub fn dispatch_script(&self, js: &str) -> Result<()> {
        self.0.send(js.to_string())?;
        Ok(())
    }
}

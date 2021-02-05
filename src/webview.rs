use crate::platform::{InnerWebView, CALLBACKS};
use crate::{Error, Result};

use std::cell::RefCell;
use std::sync::mpsc::{channel, Receiver, Sender};

use url::Url;

#[cfg(target_os = "linux")]
use gtk::Window;
#[cfg(target_os = "windows")]
use winit::platform::windows::WindowExtWindows;
#[cfg(not(target_os = "linux"))]
use winit::window::Window;

const DEBUG: bool = true;

enum Content {
    URL(Url),
    HTML(Url),
}

pub struct WebViewBuilder {
    inner: WebView,
    content: Option<Content>,
}

impl WebViewBuilder {
    pub fn new(window: Window) -> Result<Self> {
        Ok(Self {
            inner: WebView::new(window)?,
            content: None,
        })
    }

    pub fn init(self, js: &str) -> Result<Self> {
        self.inner.webview.init(js)?;
        Ok(self)
    }

    pub fn dispatch_sender(&self) -> Dispatcher {
        Dispatcher(self.inner.tx.clone())
    }

    // TODO implement bind here
    pub fn bind<F>(self, name: &str, f: F) -> Result<Self>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Send + 'static,
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
        CALLBACKS
            .lock()
            .unwrap()
            .insert(name.to_string(), Box::new(f));
        Ok(self)
    }

    pub fn load_url(mut self, url: &str) -> Result<Self> {
        self.content = Some(Content::URL(Url::parse(url)?));
        Ok(self)
    }

    pub fn load_html(mut self, html: &str) -> Result<Self> {
        let url = match Url::parse(html) {
            Ok(url) => url,
            Err(_) => Url::parse(&format!("data:text/html,{}", html))?,
        };
        self.content = Some(Content::HTML(url));
        Ok(self)
    }

    pub fn build(self) -> Result<WebView> {
        if let Some(url) = self.content {
            match url {
                Content::HTML(url) => self.inner.webview.navigate_to_string(url.as_str())?,
                Content::URL(url) => self.inner.webview.navigate(url.as_str())?,
            }
        }
        Ok(self.inner)
    }
}

thread_local!(static EVAL: RefCell<Option<Receiver<String>>> = RefCell::new(None));

pub struct WebView {
    window: Window,
    webview: InnerWebView,
    tx: Sender<String>,
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
        EVAL.with(|e| {
            *e.borrow_mut() = Some(rx);
        });
        Ok(Self {
            window,
            webview,
            tx,
        })
    }

    pub fn dispatch(&mut self, js: &str) -> Result<()> {
        self.tx.send(js.to_string())?;
        Ok(())
    }

    pub fn dispatch_sender(&self) -> Dispatcher {
        Dispatcher(self.tx.clone())
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn evaluate(&mut self) -> Result<()> {
        EVAL.with(|e| -> Result<()> {
            let e = &*e.borrow();
            if let Some(rx) = e {
                while let Ok(js) = rx.try_recv() {
                    self.webview.eval(&js)?;
                }
            } else {
                return Err(Error::EvalError);
            }
            Ok(())
        })?;

        Ok(())
    }

    pub fn resize(&self) {
        #[cfg(target_os = "windows")]
        self.webview.resize(self.window.hwnd());
    }
}

pub struct Dispatcher(Sender<String>);

impl Dispatcher {
    pub fn send(&self, js: &str) -> Result<()> {
        self.0.send(js.to_string())?;
        Ok(())
    }
}

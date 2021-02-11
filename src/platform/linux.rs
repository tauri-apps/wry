use crate::platform::{CALLBACKS, RPC};
use crate::{Dispatcher, Error, Result};

use std::rc::Rc;

use gio::Cancellable;
use gtk::{ApplicationWindow as Window, ApplicationWindowExt, ContainerExt, WidgetExt};
use webkit2gtk::{
    SettingsExt, UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserScript,
    UserScriptInjectionTime, WebView, WebViewExt,
};

pub struct InnerWebView {
    webview: Rc<WebView>,
    window_id: i64,
}

impl InnerWebView {
    pub fn new(window: &Window, debug: bool) -> Self {
        // Initialize webview widget
        let manager = UserContentManager::new();
        let webview = Rc::new(WebView::with_user_content_manager(&manager));

        let wv = Rc::clone(&webview);
        manager.register_script_message_handler("external");
        let window_id = window.get_id() as i64;
        manager.connect_script_message_received(move |_m, msg| {
            if let Some(js) = msg.get_value() {
                if let Some(context) = msg.get_global_context() {
                    if let Some(js) = js.to_string(&context) {
                        let v: RPC = serde_json::from_str(&js).unwrap();
                        let mut hashmap = CALLBACKS.lock().unwrap();
                        let (f, d) = hashmap.get_mut(&(window_id, v.method)).unwrap();
                        let status = f(d, v.id, v.params);

                        let js = match status {
                            0 => {
                                format!(
                                    r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                                    v.id, v.id
                                )
                            }
                            _ => {
                                format!(
                                    r#"window._rpc[{}].reject("RPC call fail"); window._rpc[{}] = undefined"#,
                                    v.id, v.id
                                )
                            }
                        };

                        let cancellable: Option<&Cancellable> = None;
                        wv.run_javascript(&js, cancellable, |_| ());
                    }
                }
            }
        });

        let cancellable: Option<&Cancellable> = None;
        webview.run_javascript("window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}", cancellable, |_| ());

        window.add(&*webview);
        webview.grab_focus();

        // Enable webgl and canvas features.
        if let Some(settings) = WebViewExt::get_settings(&*webview) {
            settings.set_enable_webgl(true);
            settings.set_enable_accelerated_2d_canvas(true);
            settings.set_javascript_can_access_clipboard(true);

            if debug {
                settings.set_enable_write_console_messages_to_stdout(true);
                settings.set_enable_developer_extras(true);
            }
        }

        if window.get_visible() {
            window.show_all();
        }

        Self {
            webview,
            window_id: window.get_id() as i64,
        }
    }

    pub fn init(&self, js: &str) -> Result<()> {
        if let Some(manager) = self.webview.get_user_content_manager() {
            let script = UserScript::new(
                js,
                UserContentInjectedFrames::TopFrame,
                UserScriptInjectionTime::Start,
                &[],
                &[],
            );
            manager.add_script(&script);
        } else {
            return Err(Error::InitScriptError);
        }
        Ok(())
    }

    pub fn add_callback<F>(&self, name: &str, f: F, dispatcher: Dispatcher)
    where
        F: FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send + 'static,
    {
        CALLBACKS.lock().unwrap().insert(
            (self.window_id, name.to_string()),
            (Box::new(f), dispatcher),
        );
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        let cancellable: Option<&Cancellable> = None;
        self.webview.run_javascript(js, cancellable, |_| ());
        Ok(())
    }

    pub fn navigate(&self, url: &str) -> Result<()> {
        self.webview.load_uri(url);
        Ok(())
    }

    pub fn navigate_to_string(&self, url: &str) -> Result<()> {
        self.webview.load_uri(url);
        Ok(())
    }
}

unsafe impl Send for InnerWebView {}
unsafe impl Sync for InnerWebView {}

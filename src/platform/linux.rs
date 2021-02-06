use crate::platform::{CALLBACKS, RPC};
use crate::{Error, Result};

use std::rc::Rc;

use gio::Cancellable;
use glib::Bytes;
use gtk::{ContainerExt, WidgetExt, Window};
use webkit2gtk::{
    SecurityManagerExt, SettingsExt, URISchemeRequestExt, UserContentInjectedFrames,
    UserContentManager, UserContentManagerExt, UserScript, UserScriptInjectionTime, WebContext,
    WebContextExt, WebView, WebViewExt, WebViewExtManual,
};

pub struct InnerWebView {
    webview: Rc<WebView>,
    context: WebContext,
}

impl InnerWebView {
    pub fn new(window: &Window, debug: bool) -> Self {
        // Initialize webview widget
        let manager = UserContentManager::new();
        let context = WebContext::new();
        let webview = Rc::new(WebView::new_with_context_and_user_content_manager(
            &context, &manager,
        ));

        let wv = Rc::clone(&webview);
        manager.register_script_message_handler("external");
        manager.connect_script_message_received(move |_m, msg| {
            if let Some(js) = msg.get_value() {
                if let Some(context) = msg.get_global_context() {
                    if let Some(js) = js.to_string(&context) {
                        let v: RPC = serde_json::from_str(&js).unwrap();
                        let mut hashmap = CALLBACKS.lock().unwrap();
                        let f = hashmap.get_mut(&v.method).unwrap();
                        let status = f(v.id, v.params);

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

        window.show_all();

        Self { webview, context }
    }

    pub fn register_buffer_protocol<F: 'static + Fn(&str) -> Vec<u8>>(
        &self,
        protocol: String,
        handler: F,
    ) -> Result<()> {
        self.context
            .get_security_manager()
            .unwrap()
            .register_uri_scheme_as_secure(&protocol);
        self.context
            .register_uri_scheme(&protocol.clone(), move |request| {
                let file_path = request
                    .get_uri()
                    .unwrap()
                    .as_str()
                    .replace(format!("{}://", protocol).as_str(), "")
                    // Somehow other assets get index.html in their path
                    .replace("index.html/", "");
                let mime = mime_guess::from_path(&file_path)
                    .first()
                    .unwrap()
                    .to_string();
                let buffer = handler(&file_path);
                let input = gio::MemoryInputStream::from_bytes(&Bytes::from(&buffer));
                request.finish(&input, buffer.len() as i64, Some(&mime))
            });
        Ok(())
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

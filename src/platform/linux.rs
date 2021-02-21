use crate::platform::{CALLBACKS, RPC};
use crate::webview::WV;
use crate::{Error, Result};

use std::rc::Rc;

use gdk::RGBA;
use gio::Cancellable;
use gtk::{ApplicationWindow as Window, ApplicationWindowExt, ContainerExt, WidgetExt};
use url::Url;
use webkit2gtk::{
    SettingsExt, UserContentInjectedFrames, UserContentManager, UserContentManagerExt, UserScript,
    UserScriptInjectionTime, WebView, WebViewExt,
};

pub struct InnerWebView {
    webview: Rc<WebView>,
}

impl WV for InnerWebView {
    type Window = Window;

    fn new(
        window: &Window,
        debug: bool,
        scripts: Vec<String>,
        url: Option<Url>,
        transparent: bool,
    ) -> Result<Self> {
        // Webview widget
        let manager = UserContentManager::new();
        let webview = Rc::new(WebView::with_user_content_manager(&manager));

        // Message handler
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
                            Ok(()) => {
                                format!(
                                    r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                                    v.id, v.id
                                )
                            }
                            Err(e) => {
                                format!(
                                    r#"window._rpc[{}].reject("RPC call fail with error {}"); window._rpc[{}] = undefined"#,
                                    v.id, e, v.id
                                )
                            }
                        };

                        let cancellable: Option<&Cancellable> = None;
                        wv.run_javascript(&js, cancellable, |_| ());
                    }
                }
            }
        });

        window.add(&*webview);
        webview.grab_focus();

        // Enable webgl, webaudio, canvas features and others as default.
        if let Some(settings) = WebViewExt::get_settings(&*webview) {
            settings.set_enable_webgl(true);
            settings.set_enable_webaudio(true);
            settings.set_enable_accelerated_2d_canvas(true);
            settings.set_javascript_can_access_clipboard(true);

            // Enable App cache
            settings.set_enable_offline_web_application_cache(true);
            settings.set_enable_page_cache(true);

            // Enable Smooth scrooling
            settings.set_enable_smooth_scrolling(true);

            if debug {
                settings.set_enable_write_console_messages_to_stdout(true);
                settings.set_enable_developer_extras(true);
            }
        }

        // Transparent
        if transparent {
            webview.set_background_color(&RGBA {
                red: 0.,
                green: 0.,
                blue: 0.,
                alpha: 0.,
            });
        }

        if window.get_visible() {
            window.show_all();
        }

        let w = Self { webview };

        // Initialize scripts
        w.init("window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}")?;
        for js in scripts {
            w.init(&js)?;
        }

        // Navigation
        if let Some(url) = url {
            w.webview.load_uri(url.as_str());
        }

        Ok(w)
    }

    fn eval(&self, js: &str) -> Result<()> {
        let cancellable: Option<&Cancellable> = None;
        self.webview.run_javascript(js, cancellable, |_| ());
        Ok(())
    }
}

impl InnerWebView {
    fn init(&self, js: &str) -> Result<()> {
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
}

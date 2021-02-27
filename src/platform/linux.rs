use crate::platform::{FuncCall, RpcRequest, RpcResponse, CALLBACKS, RPC, RPC_CALLBACK_NAME};
use crate::webview::WV;
use crate::{Dispatcher, Error, Result};

use std::rc::Rc;

use serde_json::Value;
use gdk::RGBA;
use gio::Cancellable;
use glib::{Bytes, FileError};
use gtk::{ApplicationWindow as Window, ApplicationWindowExt, ContainerExt, WidgetExt};
use url::Url;
use webkit2gtk::{
    SecurityManagerExt, SettingsExt, URISchemeRequestExt, UserContentInjectedFrames,
    UserContentManager, UserContentManagerExt, UserScript, UserScriptInjectionTime, WebContext,
    WebContextExt, WebView, WebViewExt, WebViewExtManual,
};

pub struct InnerWebView {
    webview: Rc<WebView>,
}

impl WV for InnerWebView {
    type Window = Window;

    fn new<F: 'static + Fn(&str) -> Result<Vec<u8>>>(
        window: &Window,
        scripts: Vec<String>,
        url: Option<Url>,
        transparent: bool,
        custom_protocol: Option<(String, F)>,
        rpc_handler: Option<(
            Dispatcher,
            Box<dyn Fn(&Dispatcher, RpcRequest) -> Option<RpcResponse> + Send>,
        )>,
    ) -> Result<Self> {
        // Webview widget
        let manager = UserContentManager::new();
        let context = WebContext::new();
        let webview = Rc::new(WebView::new_with_context_and_user_content_manager(
            &context, &manager,
        ));

        // Message handler
        let wv = Rc::clone(&webview);
        manager.register_script_message_handler("external");
        let window_id = window.get_id() as i64;
        manager.connect_script_message_received(move |_m, msg| {
            if let Some(js) = msg.get_value() {
                if let Some(context) = msg.get_global_context() {
                    if let Some(js) = js.to_string(&context) {
                        match serde_json::from_str::<FuncCall>(&js) {
                            Ok(mut ev) => {
                                // Use `isize` to conform with existing `Callback` API but should 
                                // really be a `u64`. Note that RPC spec allows for non-numbers 
                                // in the `id` field!
                                let id: i32 = if let Some(value) = ev.payload.id.take() {
                                    if let Value::Number(num) = value {
                                        if num.is_i64() { num.as_i64().unwrap() as i32 } else { 0 }
                                    } else { 0 }
                                } else { 0 };

                                let use_rpc = rpc_handler.is_some() && &ev.callback == RPC_CALLBACK_NAME;

                                // Send to an RPC handler
                                if use_rpc {
                                    let (dispatcher, rpc_handler) = rpc_handler.as_ref().unwrap();
                                    let mut response = rpc_handler(dispatcher, ev.payload);
                                    if let Some(response) = response.take() {
                                        // TODO: send response back to the client
                                    }
                                // Normal callback mechanism
                                } else {
                                    let mut hashmap = CALLBACKS.lock().unwrap();
                                    let (f, d) = hashmap.get_mut(&(window_id, ev.callback)).unwrap();
                                    // TODO: update `Callback` to take a `Value`?
                                    let raw_params = if let Some(val) = ev.payload.params.take() {
                                        val
                                    } else { Value::Null };
                                    let params = if let Value::Array(arr) = raw_params {
                                        arr 
                                    } else { vec![raw_params] };

                                    let status = f(d, id, params);
                                    let js = match status {
                                        Ok(()) => {
                                            format!(
                                                r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                                                id, id
                                            )
                                        }
                                        Err(e) => {
                                            format!(
                                                r#"window._rpc[{}].reject("RPC call fail with error {}"); window._rpc[{}] = undefined"#,
                                                id, e, id
                                            )
                                        }
                                    };

                                    let cancellable: Option<&Cancellable> = None;
                                    wv.run_javascript(&js, cancellable, |_| ());
                                }
                            }
                            Err(e) => {
                                eprintln!("Bad Javascript function call: {:?}", &js);
                            }
                        }
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

            debug_assert_eq!(
                {
                    settings.set_enable_write_console_messages_to_stdout(true);
                    settings.set_enable_developer_extras(true);
                },
                ()
            );
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

        // Custom protocol
        if let Some((name, handler)) = custom_protocol {
            context
                .get_security_manager()
                .unwrap()
                .register_uri_scheme_as_secure(&name);
            context.register_uri_scheme(&name.clone(), move |request| {
                if let Some(uri) = request.get_uri() {
                    let uri = uri.as_str();

                    match handler(uri) {
                        Ok(buffer) => {
                            let mut mime = match infer::get(&buffer) {
                                Some(info) => info.mime_type(),
                                None => "text/plain",
                            };
                            // some webtypes are technically "text/plain"
                            if mime == "text/plain" {
                                let suffix = uri.split(".").last();
                                mime = match suffix {
                                    Some("css") => "text/css",
                                    Some("html") => "text/html",
                                    Some("js") => "text/javascript",
                                    Some("json") => "application/json",
                                    Some("jsonld") => "application/ld+json",
                                    Some("ico") => "image/vnd.microsoft.icon",
                                    Some("svg") => "image/svg",
                                    Some("csv") => "text/csv",
                                    Some("rtf") => "application/rtf",
                                    // if there is something like `wry://tauri.studio`, we need to assume html
                                    Some(_) => "text/html",
                                    // using octet stream according to this:
                                    // https://developer.mozilla.org/en-US/docs/Web/HTTP/Basics_of_HTTP/MIME_types/Common_types
                                    None => "application/octet-stream",
                                };
                            }

                            let input = gio::MemoryInputStream::from_bytes(&Bytes::from(&buffer));
                            request.finish(&input, buffer.len() as i64, Some(mime))
                        }
                        Err(_) => request.finish_error(&mut glib::Error::new(
                            FileError::Exist,
                            "Could not get requested file.",
                        )),
                    }
                } else {
                    request.finish_error(&mut glib::Error::new(
                        FileError::Exist,
                        "Could not get uri.",
                    ));
                }
            });
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

use crate::platform::{CALLBACKS, RPC};
use crate::webview::WV;
use crate::Result;

use std::{
    collections::hash_map::DefaultHasher,
    ffi::{c_void, CStr, CString},
    hash::{Hash, Hasher},
    os::raw::c_char,
    ptr::null,
};

use cocoa::appkit::{NSView, NSViewHeightSizable, NSViewWidthSizable};
use cocoa::base::id;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{
    declare::ClassDecl,
    runtime::{Object, Sel},
};
use url::Url;
use winit::{platform::macos::WindowExtMacOS, window::Window};

// Safety: objc runtime calls are unsafe
unsafe fn get_nsstring(s: &str) -> id {
    let s = CString::new(s).unwrap();
    let nsstring = class!(NSString);
    msg_send![nsstring, stringWithUTF8String:s.as_ptr()]
}

pub struct InnerWebView {
    webview: id,
    manager: id,
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
        let mut hasher = DefaultHasher::new();
        window.id().hash(&mut hasher);
        let window_id = hasher.finish() as i64;

        // Callback function for message handler
        extern "C" fn did_receive(this: &Object, _: Sel, _: id, msg: id) {
            // Safety: objc runtime calls are unsafe
            unsafe {
                let window_id = *this.get_ivar("_window_id");
                let body: id = msg_send![msg, body];
                let utf8: *const c_char = msg_send![body, UTF8String];
                let s = CStr::from_ptr(utf8).to_str().expect("Invalid UTF8 string");
                let v: RPC = serde_json::from_str(&s).unwrap();
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
                let wv: id = msg_send![msg, webView];
                let js = get_nsstring(&js);
                let _: id =
                    msg_send![wv, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
            }
        }

        // Safety: objc runtime calls are unsafe
        unsafe {
            // Webview, configs, and manager
            let wkwebviewconfig = class!(WKWebViewConfiguration);
            let config: id = msg_send![wkwebviewconfig, new];
            let manager: id = msg_send![config, userContentController];
            let wkwebview = class!(WKWebView);
            let webview: id = msg_send![wkwebview, alloc];
            let preference: id = msg_send![config, preferences];
            let nsnumber = class!(NSNumber);
            let yes: id = msg_send![nsnumber, numberWithBool:1];
            let no: id = msg_send![nsnumber, numberWithBool:0];

            if debug {
                // Equivalent Obj-C:
                // [[config preferences] setValue:@YES forKey:@"developerExtrasEnabled"];
                let dev = get_nsstring("developerExtrasEnabled");
                let _: id = msg_send![preference, setValue:yes forKey:dev];
            }

            if transparent {
                // Equivalent Obj-C:
                // [config setValue:@NO forKey:@"drawsBackground"];
                let background = get_nsstring("drawsBackground");
                let _: id = msg_send![config, setValue:no forKey:background];
            }

            // Equivalent Obj-C:
            // [[config preferences] setValue:@YES forKey:@"fullScreenEnabled"];
            let fullscreen = get_nsstring("fullScreenEnabled");
            let _: id = msg_send![preference, setValue:yes forKey:fullscreen];

            // Resize
            let size = window.inner_size().to_logical(window.scale_factor());
            let rect = CGRect::new(&CGPoint::new(0., 0.), &CGSize::new(size.width, size.height));
            let _: () = msg_send![webview, initWithFrame:rect configuration:config];
            webview.setAutoresizingMask_(NSViewHeightSizable | NSViewWidthSizable);

            // Message handler
            let cls = ClassDecl::new("WebViewDelegate", class!(NSObject));
            let cls = match cls {
                Some(mut cls) => {
                    cls.add_ivar::<i64>("_window_id");
                    cls.add_method(
                        sel!(userContentController:didReceiveScriptMessage:),
                        did_receive as extern "C" fn(&Object, Sel, id, id),
                    );
                    cls.register()
                }
                None => class!(WebViewDelegate),
            };
            let handler: id = msg_send![cls, new];
            handler.as_mut().unwrap().set_ivar("_window_id", window_id);
            let external = get_nsstring("external");
            let _: () = msg_send![manager, addScriptMessageHandler:handler name:external];

            let w = Self { webview, manager };

            // Initialize scripts
            w.init(
                "window.external = {
                      invoke: function(s) {
                        window.webkit.messageHandlers.external.postMessage(s);
                      },
                    };",
            );
            for js in scripts {
                w.init(&js);
            }

            // Navigation
            if let Some(url) = url {
                if url.cannot_be_a_base() {
                    let mut s = url.as_str().split(',');
                    s.next(); // Discard scheme
                    if let Some(s) = s.next() {
                        w.navigate_to_string(s);
                    }
                } else {
                    w.navigate(url.as_str());
                }
            }

            let view = window.ns_view() as id;
            view.addSubview_(webview);

            Ok(w)
        }
    }

    fn eval(&self, js: &str) -> Result<()> {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let js = get_nsstring(js);
            let _: id = msg_send![self.webview, evaluateJavaScript:js completionHandler:null::<*const c_void>()];
        }
        Ok(())
    }
}

impl InnerWebView {
    fn init(&self, js: &str) {
        // Safety: objc runtime calls are unsafe
        // Equivalent Obj-C:
        // [manager addUserScript:[[WKUserScript alloc] initWithSource:[NSString stringWithUTF8String:js.c_str()] injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]]
        unsafe {
            let wkuserscript = class!(WKUserScript);
            let userscript: id = msg_send![wkuserscript, alloc];
            let js = get_nsstring(js);
            let script: id =
                msg_send![userscript, initWithSource:js injectionTime:0 forMainFrameOnly:1];
            let _: () = msg_send![self.manager, addUserScript: script];
        }
    }

    fn navigate(&self, url: &str) {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let nsurl = class!(NSURL);
            let s = get_nsstring(url);
            let url: id = msg_send![nsurl, URLWithString: s];
            let nsurlrequest = class!(NSURLRequest);
            let request: id = msg_send![nsurlrequest, requestWithURL: url];
            let _: () = msg_send![self.webview, loadRequest: request];
        }
    }

    fn navigate_to_string(&self, url: &str) {
        // Safety: objc runtime calls are unsafe
        unsafe {
            let nsurl = class!(NSURL);
            let html = get_nsstring(url);
            let s = get_nsstring("");
            let url: id = msg_send![nsurl, URLWithString: s];
            let _: () = msg_send![self.webview, loadHTMLString:html baseURL:url];
        }
    }
}

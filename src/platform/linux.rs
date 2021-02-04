use crate::{Error, Result};

use gio::Cancellable;
use gtk::{ContainerExt, WidgetExt, Window};
use webkit2gtk::UserContentManager;
use webkit2gtk::{
    SettingsExt, UserContentInjectedFrames, UserContentManagerExt, UserScript,
    UserScriptInjectionTime, WebView, WebViewExt,
};

pub struct InnerWebView {
    webview: WebView,
}

impl InnerWebView {
    pub fn new(window: &Window, debug: bool) -> Self {
        // Initialize webview widget
        let manager = UserContentManager::new();
        let webview = WebView::with_user_content_manager(&manager);

        /*
        webkit_user_content_manager_register_script_message_handler(
            m, // manager
            CStr::from_bytes_with_nul_unchecked(b"external\0").as_ptr(),
        );
        g_signal_connect_data(
            m as *mut _,
            CStr::from_bytes_with_nul_unchecked(b"script-message-received::external\0").as_ptr(),
            Some(std::mem::transmute(on_message as *const ())),
            w as *mut _,
            None,
            0,
        );

        webkit_web_view_run_javascript(
            webview as *mut _,
            CStr::from_bytes_with_nul_unchecked(b"window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}\0").as_ptr(),
            ptr::null_mut(),
            None,
            ptr::null_mut(),
        );
        */

        window.add(&webview);
        webview.grab_focus();

        // Enable webgl and canvas features.
        if let Some(settings) = WebViewExt::get_settings(&webview) {
            settings.set_enable_webgl(true);
            settings.set_enable_accelerated_2d_canvas(true);
            settings.set_javascript_can_access_clipboard(true);

            if debug {
                settings.set_enable_write_console_messages_to_stdout(true);
                settings.set_enable_developer_extras(true);
            }
        }

        window.show_all();

        Self { webview }
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

    pub fn bind<F>(&self, name: &str, f: F) -> Result<()>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Send + 'static,
    {
        todo!()
    }
}

/*
pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void) -> i32;
impl RawWebView {
    pub unsafe fn bind(
        webview: *mut RawWebView,
        name: &str,
        fn_: BindFn,
        arg: *mut c_void,
    ) -> Result<(), Error> {
        let name = CString::new(name)?;
        let js = format!(
            r#"(function() {{ var name = {:?};
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
}}())"#,
            name
        );
        RawWebView::init(webview, &js)?;
        (*webview).callbacks.insert(name, (fn_, arg));
        Ok(())
    }
}
unsafe extern "C" fn on_message(
    _m: *mut WebKitUserContentManager,
    r: *mut WebKitJavascriptResult,
    arg: gpointer,
) {
    #[derive(Serialize, Deserialize)]
    struct RPC {
        id: i8,
        method: CString,
        params: serde_json::Value,
    }

    let webview: *mut RawWebView = arg as *mut _;
    let ctx = webkit_javascript_result_get_global_context(r);
    let value = webkit_javascript_result_get_value(r);
    let js = JSValueToStringCopy(ctx, value, ptr::null_mut());
    let n = JSStringGetMaximumUTF8CStringSize(js);
    let mut s = Vec::<u8>::with_capacity(n);
    JSStringGetUTF8CString(js, s.as_mut_ptr() as _, n);
    s.set_len(n - 1);
    let mut c = 0;
    loop {
        if s[c] == 0 {
            break;
        }
        c += 1;
    }
    let _ = s.split_off(c);
    let v: RPC = serde_json::from_str(std::str::from_utf8(&s).unwrap()).unwrap();
    let req = CString::new(serde_json::to_string(&v.params).unwrap()).unwrap();
    if let Some((f, arg)) = (*webview).callbacks.get(&v.method) {
        let status = (*f)(&v.id, req.as_ptr(), *arg);
        match status {
            0 => {
                let js = format!(
                    r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                    v.id, v.id
                );
                RawWebView::eval(webview, &js).expect("This should be valid CString");
            }
            _ => {
                let js = format!(
                    r#"window._rpc[{}].reject("RPC call fail"); window._rpc[{}] = undefined"#,
                    v.id, v.id
                );
                RawWebView::eval(webview, &js).expect("This should be valid CString");
            }
        }
    }

    JSStringRelease(js);
}*/

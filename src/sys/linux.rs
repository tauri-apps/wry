use crate::Error;

use gdk_sys::{GdkGeometry, GDK_HINT_MAX_SIZE, GDK_HINT_MIN_SIZE};
use glib_sys::{gpointer, GFALSE};
use gobject_sys::g_signal_connect_data;
use gtk_sys::*;
use javascriptcore_sys::*;
use webkit2gtk_sys::*;

use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    os::raw::{c_char, c_int, c_void},
    ptr,
};

pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void) -> i32;

pub const WEBVIEW_HINT_NONE: c_int = 0;
pub const WEBVIEW_HINT_MIN: c_int = 1;
pub const WEBVIEW_HINT_MAX: c_int = 2;
pub const WEBVIEW_HINT_FIXED: c_int = 3;

pub struct RawWebView {
    debug: bool,
    window: *mut GtkWidget,
    webview: *mut GtkWidget,
    callbacks: HashMap<CString, (BindFn, *mut c_void)>,
}

impl RawWebView {
    pub unsafe fn new(debug: bool) -> *mut RawWebView {
        let w = Box::into_raw(Box::new(RawWebView {
            debug,
            window: ptr::null_mut(),
            webview: ptr::null_mut(),
            callbacks: HashMap::new(),
        }));

        if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == GFALSE {
            return ptr::null_mut();
        }

        let window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
        (*w).window = window;

        g_signal_connect_data(
            window as *mut _,
            CStr::from_bytes_with_nul_unchecked(b"destroy\0").as_ptr(),
            Some(gtk_main_quit),
            w as *mut _,
            None,
            0,
        );

        // Initialize webview widget
        let m = webkit_user_content_manager_new();
        let webview = webkit_web_view_new_with_user_content_manager(m);
        (*w).webview = webview;

        webkit_user_content_manager_register_script_message_handler(
            m,
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

        gtk_container_add(window as *mut _, webview);
        gtk_widget_grab_focus(webview);

        let settings = webkit_web_view_get_settings(webview as *mut _);
        // Enable webgl and canvas features.
        webkit_settings_set_enable_webgl(settings, 1);
        webkit_settings_set_enable_accelerated_2d_canvas(settings, 1);
        webkit_settings_set_javascript_can_access_clipboard(settings, 1);
        if debug {
            webkit_settings_set_enable_write_console_messages_to_stdout(settings, 1);
            webkit_settings_set_enable_developer_extras(settings, 1);
        }

        gtk_widget_show_all(window);

        w
    }

    pub unsafe fn run(webview: *mut RawWebView) {
        let _ = Box::from_raw(webview);
        gtk_main();
    }

    pub unsafe fn set_title(webview: *mut RawWebView, title: &str) -> Result<(), Error> {
        let title = CString::new(title)?;
        gtk_window_set_title((*webview).window as *mut _, title.as_ptr());
        Ok(())
    }

    pub unsafe fn set_size(
        webview: *mut RawWebView,
        width: c_int,
        height: c_int,
        hint: c_int,
    ) {
        match hint {
            WEBVIEW_HINT_FIXED => {
                gtk_window_set_resizable((*webview).window as *mut _, 0);
                gtk_widget_set_size_request((*webview).window, width, height);
            }
            WEBVIEW_HINT_NONE => {
                gtk_window_set_resizable((*webview).window as *mut _, 1);
                gtk_window_resize((*webview).window as *mut _, width, height);
            }
            hint => {
                gtk_window_set_resizable((*webview).window as *mut _, 1);
                let mut g = GdkGeometry {
                    min_width: width,
                    min_height: height,
                    max_width: width,
                    max_height: height,
                    base_width: Default::default(),
                    base_height: Default::default(),
                    width_inc: Default::default(),
                    height_inc: Default::default(),
                    min_aspect: Default::default(),
                    max_aspect: Default::default(),
                    win_gravity: Default::default(),
                };
                let h = if hint == WEBVIEW_HINT_MIN {
                    GDK_HINT_MIN_SIZE
                } else {
                    GDK_HINT_MAX_SIZE
                };
                gtk_window_set_geometry_hints((*webview).window as *mut _, ptr::null_mut(), &mut g, h);
            }
        }
    }

    pub unsafe fn navigate(webview: *mut RawWebView, url: &str) -> Result<(), Error> {
        let url = CString::new(url)?;
        webkit_web_view_load_uri((*webview).webview as *mut _, url.as_ptr());
        Ok(())
    }

    pub unsafe fn init(webview: *mut RawWebView, js: &str) -> Result<(), Error> {
        let js = CString::new(js)?;
        webkit_user_content_manager_add_script(
            webkit_web_view_get_user_content_manager((*webview).webview as *mut _),
            webkit_user_script_new(
                js.as_ptr(),
                WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
                WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
                ptr::null(),
                ptr::null(),
            ),
        );
        Ok(())
    }

    pub unsafe fn eval(webview: *mut RawWebView, js: &str) -> Result<(), Error> {
        let js = CString::new(js)?;
        webkit_web_view_run_javascript(
            (*webview).webview as *mut _,
            js.as_ptr(),
            ptr::null_mut(),
            None,
            ptr::null_mut(),
        );
        Ok(())
    }

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
}

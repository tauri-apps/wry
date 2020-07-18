use gdk_sys::{GdkGeometry, GDK_HINT_MAX_SIZE, GDK_HINT_MIN_SIZE};
use glib_sys::*;
use gobject_sys::g_signal_connect_data;
use gtk_sys::*;
use javascriptcore_sys::*;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use webkit2gtk_sys::*;

pub const WEBVIEW_HINT_NONE: c_int = 0;
pub const WEBVIEW_HINT_MIN: c_int = 1;
pub const WEBVIEW_HINT_MAX: c_int = 2;
pub const WEBVIEW_HINT_FIXED: c_int = 3;

pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void) -> i32;
pub type DispatchFn = extern "C" fn(webview: *mut Weebview, arg: *mut c_void);

#[repr(C)]
pub struct Weebview {
    debug: bool,
    window: *mut GtkWidget,
    webview: *mut GtkWidget,
    callbacks: HashMap<CString, (BindFn, *mut c_void)>,
}

#[no_mangle]
pub unsafe extern "C" fn webview_create(debug: bool, window: *mut GtkWidget) -> *mut Weebview {
    let w = Box::into_raw(Box::new(Weebview {
        debug,
        window,
        webview: ptr::null_mut(),
        callbacks: HashMap::new(),
    }));

    if gtk_init_check(ptr::null_mut(), ptr::null_mut()) == GFALSE {
        return ptr::null_mut();
    }

    let window = match (*w).window {
        w if w.is_null() => gtk_window_new(GTK_WINDOW_TOPLEVEL),
        _ => (*w).window,
    };
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

    // TODO
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

#[no_mangle]
pub unsafe extern "C" fn webview_destroy(webview: *mut Weebview) {
    let _ = Box::from_raw(webview);
}

#[no_mangle]
pub unsafe extern "C" fn webview_run(_webview: *mut Weebview) {
    gtk_main();
}

#[no_mangle]
pub unsafe extern "C" fn webview_terminate(_webview: *mut Weebview) {
    gtk_main_quit();
}

#[no_mangle]
pub unsafe extern "C" fn webview_set_title(webview: *mut Weebview, title: *const c_char) {
    gtk_window_set_title((*webview).window as *mut _, title);
}

#[no_mangle]
pub unsafe extern "C" fn webview_set_size(
    webview: *mut Weebview,
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
            gtk_window_set_geometry_hints(
                (*webview).window as *mut _,
                ptr::null_mut(),
                &mut g,
                h,
            );
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn webview_get_window(webview: *mut Weebview) -> *mut GtkWidget {
    (*webview).window
}

#[no_mangle]
pub unsafe extern "C" fn webview_navigate(webview: *mut Weebview, url: *const c_char) {
    webkit_web_view_load_uri((*webview).webview as *mut _, url);
}

#[no_mangle]
pub unsafe extern "C" fn webview_init(webview: *mut Weebview, js: *const c_char) {
    webkit_user_content_manager_add_script(
        webkit_web_view_get_user_content_manager((*webview).webview as *mut _),
        webkit_user_script_new(
            js,
            WEBKIT_USER_CONTENT_INJECT_TOP_FRAME,
            WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START,
            ptr::null(),
            ptr::null(),
        ),
    );
}

#[no_mangle]
pub unsafe extern "C" fn webview_eval(webview: *mut Weebview, js: *const c_char) {
    webkit_web_view_run_javascript(
        (*webview).webview as *mut _,
        js,
        ptr::null_mut(),
        None,
        ptr::null_mut(),
    );
}

#[no_mangle]
pub unsafe extern "C" fn webview_dispatch(
    webview: *mut Weebview,
    fn_: DispatchFn,
    arg: *mut c_void,
) {
    #[repr(C)]
    struct DispatchArg {
        fn_: DispatchFn,
        webview: *mut Weebview,
        arg: *mut c_void,
    }

    unsafe extern "C" fn cb(data: *mut c_void) -> i32 {
        let data: Box<DispatchArg> = Box::from_raw(data as *mut _);

        (data.fn_)(data.webview, data.arg);
        0
    }

    let data = Box::into_raw(Box::new(DispatchArg{fn_, webview, arg}));
    g_idle_add_full(G_PRIORITY_HIGH_IDLE, Some(cb), data as *mut _, None);

}

#[no_mangle]
pub unsafe extern "C" fn webview_bind(
    webview: *mut Weebview,
    name: *const c_char,
    fn_: BindFn,
    arg: *mut c_void,
) {
    let name = CStr::from_ptr(name).to_owned();
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
}})()"#,
        name
    );
    webview_init(webview, CString::new(js).unwrap().as_ptr());
    (*webview).callbacks.insert(name, (fn_, arg));
}

pub unsafe extern "C" fn on_message(
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

    let webview: *mut Weebview = arg as *mut _;
    let ctx = webkit_javascript_result_get_global_context(r);
    let value = webkit_javascript_result_get_value(r);
    let js = JSValueToStringCopy(ctx, value, ptr::null_mut());
    let n = JSStringGetMaximumUTF8CStringSize(js);
    let mut s = Vec::<u8>::with_capacity(n);
    JSStringGetUTF8CString(js, s.as_mut_ptr() as _, n);
    s.set_len(n - 1);
    let mut c = 0;
    loop {
        if s[c] == 0 { break; }
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
                webview_eval(webview, CString::new(js).unwrap().as_ptr());
            }
            _ => {
                let js = format!(
                    r#"window._rpc[{}].reject("RPC call fail"); window._rpc[{}] = undefined"#,
                    v.id, v.id
                );
                webview_eval(webview, CString::new(js).unwrap().as_ptr());
            }
        }
    }

    JSStringRelease(js);
}

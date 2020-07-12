use gdk_sys::{GdkGeometry, GDK_HINT_MIN_SIZE, GDK_HINT_MAX_SIZE};
use glib_sys::*;
use gobject_sys::g_signal_connect_data;
use gtk_sys::*;
use javascriptcore_sys::*;
use std::ffi::{CStr, CString};
use std::mem;
use std::os::raw::{c_char, c_int, c_void};
use std::ptr;
use webkit2gtk_sys::*;

pub const WEBVIEW_HINT_NONE: c_int = 0;
pub const WEBVIEW_HINT_MIN: c_int = 1;
pub const WEBVIEW_HINT_MAX: c_int = 2;
pub const WEBVIEW_HINT_FIXED: c_int = 3;

pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void);
pub type DispatchFn = extern "C" fn(webview: *mut WebView, arg: *mut c_void);

#[repr(C)]
pub struct WebView {
    debug: bool,
    window: *mut GtkWidget,
    webview: *mut GtkWidget,
}


#[no_mangle]
pub unsafe extern "C" fn webview_create(
    debug: bool,
    window: *mut GtkWidget,
) -> *mut WebView {
    let w = Box::into_raw(Box::new(WebView {
        debug,
        window,
        webview: ptr::null_mut(),
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
        mem::transmute(window),
        CStr::from_bytes_with_nul_unchecked(b"destroy\0").as_ptr(),
        Some(gtk_main_quit),
        mem::transmute(w),
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
        mem::transmute(m),
        CStr::from_bytes_with_nul_unchecked(b"script-message-received::external\0").as_ptr(),
        Some(mem::transmute(external_message_received_cb as *const ())), // TODO call onmessage which search the callback hashmap
        mem::transmute(w),
        None,
        0,
    );

    // TODO
    webkit_web_view_run_javascript(
        mem::transmute(webview),
        CStr::from_bytes_with_nul_unchecked(b"window.external={invoke:function(x){window.webkit.messageHandlers.external.postMessage(x);}}\0").as_ptr(),
        ptr::null_mut(),
        None,
        ptr::null_mut(),
    );

    gtk_container_add(mem::transmute(window), webview);
    gtk_widget_grab_focus(webview);

    let settings = webkit_web_view_get_settings(mem::transmute(webview));
    // Enable webgl and canvas features.
    webkit_settings_set_enable_webgl(settings, 1);
    webkit_settings_set_enable_accelerated_2d_canvas(settings, 1);
    if debug {
        webkit_settings_set_enable_write_console_messages_to_stdout(settings, 1);
        webkit_settings_set_enable_developer_extras(settings, 1);
    }

    gtk_widget_show_all(window);

    w
}

#[no_mangle]
pub unsafe extern "C" fn webview_destroy(webview: *mut WebView) {
    let _ = Box::from_raw(webview);
}

#[no_mangle]
pub unsafe extern "C" fn webview_run(_webview: *mut WebView) {
    gtk_main();
}

#[no_mangle]
pub unsafe extern "C" fn webview_terminate(_webview: *mut WebView) {
    gtk_main_quit();
}

#[no_mangle]
pub unsafe extern "C" fn webview_set_title(webview: *mut WebView, title: *const c_char) {
    gtk_window_set_title(mem::transmute((*webview).window), title);
}

#[no_mangle]
pub unsafe extern "C" fn webview_set_size(webview: *mut WebView, width: c_int, height: c_int, hint: c_int) {
    match hint {
        WEBVIEW_HINT_FIXED => {
            gtk_window_set_resizable(mem::transmute((*webview).window), 0);
            gtk_widget_set_size_request((*webview).window, width, height);
        },
        WEBVIEW_HINT_NONE => {
            gtk_window_set_resizable(mem::transmute((*webview).window), 1);
            gtk_window_resize(mem::transmute((*webview).window), width, height);
        }
        hint => {
            gtk_window_set_resizable(mem::transmute((*webview).window), 1);
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
            let h = if hint == WEBVIEW_HINT_MIN {GDK_HINT_MIN_SIZE } else {GDK_HINT_MAX_SIZE};
            gtk_window_set_geometry_hints(mem::transmute((*webview).window), ptr::null_mut(), &mut g, h);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn webview_get_window(webview: *mut WebView) -> *mut GtkWidget {
    (*webview).window
}

#[no_mangle]
pub unsafe extern "C" fn webview_navigate(webview: *mut WebView, url: *const c_char) {
    webkit_web_view_load_uri(mem::transmute((*webview).webview), url);
}

#[no_mangle]
pub unsafe extern "C" fn webview_init(webview: *mut WebView, js: *const c_char) {
    let m = webkit_web_view_get_user_content_manager(mem::transmute((*webview).webview));
    webkit_user_content_manager_add_script(m, webkit_user_script_new(js, WEBKIT_USER_CONTENT_INJECT_TOP_FRAME, WEBKIT_USER_SCRIPT_INJECT_AT_DOCUMENT_START, ptr::null(), ptr::null()));
}

#[no_mangle]
pub unsafe extern "C" fn webview_eval(webview: *mut WebView, js: *const c_char) {
    webkit_web_view_run_javascript(mem::transmute((*webview).webview), js, ptr::null_mut(), None, ptr::null_mut());
}

#[no_mangle]
pub unsafe extern "C" fn webview_dispatch(webview: *mut WebView, fn_: DispatchFn, arg: *mut c_void) {
    fn_(webview, arg);
}

#[no_mangle]
pub unsafe extern "C" fn webview_bind(webview: *mut WebView, name: *const c_char, fn_: BindFn, arg: *mut c_void) {
    let name = CStr::from_ptr(name);
    let js = format!("(function() {{ var name = ' {:?} '; (
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
      }})())", name);
    webview_init(webview, CString::new(js).unwrap().as_ptr());
    // TODO Register callback hashmap
}


#[no_mangle]
pub unsafe extern "C" fn webview_return(webview: *mut WebView, seq: *const c_char, status: c_int, result: *const c_char) {
    let seq = CStr::from_ptr(seq);
    let result = CStr::from_ptr(result);
    match status {
        0 => {
            let js = format!("window._rpc[ {:?} ].resolve( {:?} ); window._rpc[ {:?} ] = undefined", seq , result, seq);
            webview_eval(webview, CString::new(js).unwrap().as_ptr());
        },
        _ => {
            let js = format!("window._rpc[ {:?} ].reject( {:?} ); window._rpc[ {:?} ] = undefined", seq , result, seq);
            webview_eval(webview, CString::new(js).unwrap().as_ptr());
        },
    }
}

pub unsafe extern "C" fn external_message_received_cb(
    _m: *mut WebKitUserContentManager,
    r: *mut WebKitJavascriptResult,
    arg: gpointer,
) {
    let webview: *mut WebView = mem::transmute(arg);
    let ctx = webkit_javascript_result_get_global_context(r);
    let value = webkit_javascript_result_get_value(r);
    let js = JSValueToStringCopy(ctx, value, ptr::null_mut());
    let n = JSStringGetMaximumUTF8CStringSize(js);
    let mut s = Vec::new();
    s.reserve(n);
    JSStringGetUTF8CString(js, s.as_mut_ptr(), n);
    //((*webview).external_invoke_cb)(webview, s.as_ptr());
}

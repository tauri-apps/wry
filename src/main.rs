use weebview::*;
use std::ptr;
use std::ffi::CString;
use std::os::raw::c_void;

fn main() { unsafe {
    let data = webview_create(true, ptr::null_mut());
    webview_set_title(data, CString::new("TEST").unwrap().as_ptr());
    webview_set_size(data, 1024, 768, 0);
    webview_init(data, CString::new("window.x = 42").unwrap().as_ptr());
    webview_dispatch(data, test, ptr::null_mut());
    webview_navigate(data, CString::new("https://google.com").unwrap().as_ptr());
    webview_run(data);
}}

#[no_mangle]
extern "C" fn test(webview: *mut WebView, _arg: *mut c_void) {
    unsafe { webview_set_size(webview, 800, 600, 1); }
    println!("Hello World");
}
use weebview::*;


// use winit::{
//     event::{Event, StartCause, WindowEvent},
//     event_loop::{ControlFlow, EventLoop},
//     window::Window,
// };

use std::ffi::CString;
use std::os::raw::{c_char, c_void};
use std::ptr;


//use winit::platform::windows::WindowExtWindows;

fn main() -> Result<(), Box<dyn std::error::Error>>{
    let mut webview = WebView::new(true)?;
    webview.init("window.x = 42")?;
    
    //let w = webview.clone();
    webview.bind("xxx", |_seq, _req| {
        // match w.eval("console.log('The anwser is ' + window.x);").is_ok() {
        //     true => 0,
        //     false => 1,
        // }
        println!("Hello");
        0
    })?;
    webview.navigate("https://www.google.com")?;
    webview.run();
    Ok(())
    // unsafe {
    //     let data = webview_create(true);
    //     webview_set_title(data, CString::new("AYAYA").unwrap().as_ptr());
    //     webview_set_size(data, 1024, 768, 0);
    //     webview_init(data, CString::new("window.x = 42").unwrap().as_ptr());
    //     webview_dispatch(data, dispatch, ptr::null_mut());
    //     webview_bind(
    //         data,
    //         CString::new("UwU").unwrap().as_ptr(),
    //         bind,
    //         ptr::null_mut(),
    //     );
    //     webview_navigate(
    //         data,
    //         CString::new("https://downloadmorerem.com/")
    //             .unwrap()
    //             .as_ptr(),
    //     );
    //     webview_run(data);
    // }
}

// #[no_mangle]
// extern "C" fn dispatch(webview: *mut RawWebview, _arg: *mut c_void) {
//     unsafe {
//         webview_set_size(webview, 800, 600, 1);
//     }
//     println!("Download More Rem!");
// }

// #[no_mangle]
// extern "C" fn bind(seq: *const c_char, _req: *const c_char, _arg: *mut c_void) -> i32 {
//     unsafe {
//         println!("{}", *seq);
//     }
//     0i32
// }

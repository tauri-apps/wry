include!(concat!(env!("OUT_DIR"), "/winrt.rs"));

use crate::Error;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::size_of;
use std::os::raw::{c_char, c_void};
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

use serde::{Deserialize, Serialize};
use winapi::{
    shared::{minwindef::*, windef::*},
    um::{
        combaseapi::*,
        libloaderapi::*,
        synchapi::{CreateEventW, SetEvent},
        winbase::INFINITE,
        winnt::LPCWSTR,
        winuser::*,
    },
    winrt::roapi::{RoInitialize, RO_INIT_SINGLETHREADED},
};
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::Window,
};

use windows::foundation::*;
use windows::web::ui::interop::*;
use windows::web::ui::*;
use winrt::HString;

pub struct WebView{
    events: EventLoop<()>,
    window: Window,
    webview: WebViewControl,
}

impl WebView {
    pub fn new(debug: bool) -> Result<Self, Error> {
        let events = EventLoop::new();
        let window = Window::new(&events)?;
        let op = WebViewControlProcess::new()?
            .create_web_view_control_async(window.hwnd() as i64, Rect::default())?;

        if op.status()? != AsyncStatus::Completed {
            let h = unsafe { CreateEventW(null_mut(), 0i32, 0i32, null()) };
            let mut hs = h.clone();
            op.set_completed(AsyncOperationCompletedHandler::new(move |_, _| {
                unsafe {
                    SetEvent(h);
                }
                Ok(())
            }))?;
            unsafe {
                CoWaitForMultipleHandles(
                    COWAIT_DISPATCH_WINDOW_MESSAGES | COWAIT_DISPATCH_CALLS | COWAIT_INPUTAVAILABLE,
                    INFINITE,
                    1,
                    &mut hs,
                    &mut 0u32,
                );
            }
        }

        let webview = op.get_results()?;
        webview.settings()?.set_is_script_notify_allowed(true)?;
        webview.set_is_visible(true)?;
        webview.script_notify(TypedEventHandler::new(
            |_, args: &WebViewControlScriptNotifyEventArgs| {
                let s = args.value()?.to_string();
                dbg!(s);
                // TODO call on message
                Ok(())
            },
        ))?;
        
        let w = WebView {
            events,
            window,
            webview,
        };
        resize(&w.webview, w.window.hwnd() as *mut _);

        Ok(w)
    }

    pub fn navigate(&self, url: &str) -> Result<(), Error> {
        self.webview.navigate(Uri::create_uri(url)?)?;
        Ok(())
        // std::string html = html_from_uri(url);
        // if (html != "") {
        //   m_webview.NavigateToString(winrt::to_hstring(html));
        // } else {
        //   Uri uri(winrt::to_hstring(url));
        //   m_webview.Navigate(uri);
        // }
    }

    pub fn init(&self, js: &str) -> Result<(), Error> {
        let script = String::from("(function(){") + js + "})();";
        self.webview.add_initialize_script(script)?;
        Ok(())
    }

    pub fn eval(&self, js: &str) -> Result<(), Error> {
        //self.webview.invoke_script_async("name", vec![HString::from(js)].into_iter())?;
        Ok(())
    }

    pub fn run(self) {
        let window = self.window;
        let webview = self.webview;
        self.events.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;

            match event {
                Event::NewEvents(StartCause::Init) => {}
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => *control_flow = ControlFlow::Exit,
                Event::WindowEvent {
                    event: WindowEvent::Resized(_),
                    ..
                } => {
                    resize(&webview, window.hwnd() as *mut _);
                }
                _ => (),
            }
        });
    }

    
}

fn resize(webview: &WebViewControl, wnd: HWND) {
    unsafe {
        if wnd.is_null() {
            return;
        }
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        GetClientRect(wnd, &mut r);
        let r = Rect {
            x: r.left as f32,
            y: r.top as f32,
            width: (r.right - r.left) as f32,
            height: (r.bottom - r.top) as f32,
        };

        webview.set_bounds(r).unwrap();
    }
}

// pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void) -> i32;
// pub type DispatchFn = extern "C" fn(webview: *mut Weebview, arg: *mut c_void);

// #[repr(C)]
// pub struct Weebview {
//     // debug: bool,
//     window: HWND,
//     webview: Option<WebViewControl>,
//     callbacks: HashMap<String, (BindFn, *mut c_void)>,
//     inject_js: String,
// }

// pub fn new() -> Result<*mut Weebview, Error> {
//     unsafe {
//         // TODO init_apartment(winrt::apartment_type::single_threaded);
//         RoInitialize(RO_INIT_SINGLETHREADED);
//         let w = Box::into_raw(Box::new(Weebview {
//             //debug,
//             window: null_mut(),
//             webview: None,
//             callbacks: HashMap::new(),
//             inject_js: String::new(),
//         }));

//         (*w).window = {
//             let hinstance = GetModuleHandleW(null());
//             let icon = LoadImageW(
//                 hinstance,
//                 IDI_APPLICATION,
//                 IMAGE_ICON,
//                 GetSystemMetrics(SM_CXSMICON),
//                 GetSystemMetrics(SM_CYSMICON),
//                 LR_DEFAULTCOLOR,
//             ) as HICON;

//             unsafe extern "system" fn proc(
//                 hwnd: HWND,
//                 msg: UINT,
//                 wp: WPARAM,
//                 lp: LPARAM,
//             ) -> LRESULT {
//                 match msg {
//                     WM_SIZE => {
//                         let wv = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Weebview;
//                         resize(wv, hwnd);
//                     }
//                     WM_CLOSE => {
//                         DestroyWindow(hwnd);
//                     }
//                     WM_DESTROY => {
//                         PostQuitMessage(0);
//                     }
//                     _ => (),
//                 }

//                 DefWindowProcW(hwnd, msg, wp, lp)
//             }

//             let wx = WNDCLASSEXW {
//                 cbSize: size_of::<WNDCLASSEXW>() as u32,
//                 style: 0,
//                 lpfnWndProc: Some(proc),
//                 cbClsExtra: 0,
//                 cbWndExtra: 0,
//                 hInstance: hinstance,
//                 hIcon: icon,
//                 hCursor: null_mut(),
//                 hbrBackground: 0 as HBRUSH,
//                 lpszMenuName: null(),
//                 lpszClassName: OsStr::new("webview")
//                     .encode_wide()
//                     .collect::<Vec<u16>>()
//                     .as_ptr(),
//                 hIconSm: icon,
//             };
//             RegisterClassExW(&wx);

//             CreateWindowExW(
//                 0,
//                 OsStr::new("webview")
//                     .encode_wide()
//                     .collect::<Vec<u16>>()
//                     .as_ptr(),
//                 0 as LPCWSTR,
//                 WS_OVERLAPPEDWINDOW,
//                 CW_USEDEFAULT,
//                 CW_USEDEFAULT,
//                 CW_USEDEFAULT,
//                 CW_USEDEFAULT,
//                 null_mut(),
//                 null_mut(),
//                 hinstance,
//                 null_mut(),
//             )
//         };

//         // Initialize webview
//         let op = WebViewControlProcess::new()
//             .expect("it's meeeee")
//             .create_web_view_control_async((*w).window as i64, Rect::default())
//             .unwrap();

//         if op.status().unwrap() != AsyncStatus::Completed {
//             let h = CreateEventW(null_mut(), 0i32, 0i32, null());
//             let mut hs = h.clone();
//             op.set_completed(AsyncOperationCompletedHandler::new(move |_, _| {
//                 SetEvent(h);
//                 Ok(())
//             }))
//             .unwrap();
//             CoWaitForMultipleHandles(
//                 COWAIT_DISPATCH_WINDOW_MESSAGES | COWAIT_DISPATCH_CALLS | COWAIT_INPUTAVAILABLE,
//                 INFINITE,
//                 1,
//                 &mut hs,
//                 &mut 0u32,
//             );
//         }

//         let webview = op.get_results().unwrap();
//         webview
//             .settings()
//             .unwrap()
//             .set_is_script_notify_allowed(true)
//             .unwrap();
//         webview.set_is_visible(true).unwrap();
//         webview
//             .script_notify(TypedEventHandler::new(
//                 |_, args: &WebViewControlScriptNotifyEventArgs| {
//                     let s = args.value().unwrap().to_string();
//                     #[derive(Serialize, Deserialize)]
//                     struct RPC {
//                         id: i8,
//                         method: String,
//                         params: serde_json::Value,
//                     }
//                     let v: RPC = serde_json::from_str(&s).unwrap();
//                     // if let Some((f, arg)) = (*webview).callbacks.get(&v.method) {
//                     //     let status = (*f)(&v.id, req.as_ptr(), *arg);
//                     // match status {
//                     //     0 => {
//                     //         let js = format!(
//                     //             r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
//                     //             v.id, v.id
//                     //         );
//                     //         webview_eval(webview, CString::new(js).unwrap().as_ptr());
//                     //     }
//                     //     _ => {
//                     //         let js = format!(
//                     //             r#"window._rpc[{}].reject("RPC call fail"); window._rpc[{}] = undefined"#,
//                     //             v.id, v.id
//                     //         );
//                     //         webview_eval(webview, CString::new(js).unwrap().as_ptr());
//                     //     }
//                     // }
//                     // }
//                     Ok(())
//                 },
//             ))
//             .unwrap();
//         let wv = webview.clone();
//         webview
//             .navigation_starting(TypedEventHandler::new(move |_, _| {
//                 wv.add_initialize_script(HString::from((*w).inject_js.clone()))
//                     .unwrap();
//                 Ok(())
//             }))
//             .unwrap();
//         init(w, "window.external.invoke = s => window.external.notify(s)");
//         (*w).webview = Some(webview);

//         SetWindowLongPtrW((*w).window, GWLP_USERDATA, w as isize);
//         ShowWindow((*w).window, SW_SHOW);
//         UpdateWindow((*w).window);
//         SetFocus((*w).window);

//         resize(w, (*w).window);

//         Ok(w)
//     }
// }

// unsafe fn resize(w: *mut Weebview, wnd: HWND) {
//     if wnd.is_null() {
//         return;
//     }
//     let mut r = RECT {
//         left: 0,
//         top: 0,
//         right: 0,
//         bottom: 0,
//     };
//     GetClientRect(wnd, &mut r);
//     let r = Rect {
//         x: r.left as f32,
//         y: r.top as f32,
//         width: (r.right - r.left) as f32,
//         height: (r.bottom - r.top) as f32,
//     };

//     if let Some(webview) = &(*w).webview {
//         webview.set_bounds(r).unwrap();
//     }

// }

// pub unsafe fn navigate(w: *mut Weebview, url: &str) {
//     if let Some(webview) = &(*w).webview {
//         webview.navigate(Uri::create_uri(url).unwrap());
//     }
//     // std::string html = html_from_uri(url);
//     // if (html != "") {
//     //   m_webview.NavigateToString(winrt::to_hstring(html));
//     // } else {
//     //   Uri uri(winrt::to_hstring(url));
//     //   m_webview.Navigate(uri);
//     // }
// }

// pub unsafe fn init(w: *mut Weebview, js: &str) {
//     (*w).inject_js += js;
// }

// pub fn run(w: *mut Weebview) {
//     unsafe {
//         let mut msg: MSG = std::mem::zeroed();
//         // TODO PeekMessage for non blocking
//         while GetMessageA(&mut msg, null_mut(), 0, 0) != -1 {
//             if !msg.hwnd.is_null() {
//                 TranslateMessage(&msg);
//                 DispatchMessageA(&msg);
//                 continue;
//             }

//             if msg.message == WM_APP {
//                 let f = msg.lParam as *mut extern "C" fn();
//                 (*f)();
//                 libc::free(f as *mut _);
//             } else if msg.message == WM_QUIT {
//                 return;
//             }
//         }
//     }
// }

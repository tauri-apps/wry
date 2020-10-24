use crate::Error;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::mem::size_of;
use std::os::raw::{c_char, c_void};
use std::ffi::{CStr, CString};
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};
use std::sync::Mutex;
use std::marker::{Sync, Send};

use once_cell::sync::Lazy;
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

winrt::import!(
    dependencies
        os
    types
        windows::foundation::*
        windows::web::ui::*
        windows::web::ui::interop::*
);

use windows::foundation::*;
use windows::foundation::collections::*;
use windows::web::ui::interop::*;
use windows::web::ui::*;
use winrt::HString;

static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn FnMut(i8, Vec<String>) -> i32 + Sync + Send>>>> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

pub type BindFn = extern "C" fn(seq: *const c_char, req: *const c_char, arg: *mut c_void) -> i32;

#[derive(Debug, Serialize, Deserialize)]
struct RPC {
    id: i8,
    method: String,
    params: Vec<String>,
}

pub struct RawWebView{
    events: EventLoop<()>,
    window: Window,
    webview: WebViewControl,
}

impl RawWebView {
    pub unsafe fn new(_debug: bool) -> Result<*mut RawWebView, Error> {
        let events = EventLoop::new();
        let window = Window::new(&events)?;
        let op = WebViewControlProcess::new()?
            .create_web_view_control_async(window.hwnd() as i64, Rect::default())?;

        if op.status()? != AsyncStatus::Completed {
            let h = CreateEventW(null_mut(), 0i32, 0i32, null());
            let mut hs = h.clone();
            op.set_completed(AsyncOperationCompletedHandler::new(move |_, _| {
                SetEvent(h);
                Ok(())
            }))?;

            CoWaitForMultipleHandles(
                COWAIT_DISPATCH_WINDOW_MESSAGES | COWAIT_DISPATCH_CALLS | COWAIT_INPUTAVAILABLE,
                INFINITE,
                1,
                &mut hs,
                &mut 0u32,
            );
        }

        let webview = op.get_results()?;
        webview.settings()?.set_is_script_notify_allowed(true)?;
        webview.set_is_visible(true)?;

        let w = Box::into_raw(Box::new(RawWebView {
            events,
            window,
            webview,
        }));

        (*w).webview.script_notify(TypedEventHandler::new(
            |wv: &IWebViewControl, args: &WebViewControlScriptNotifyEventArgs| {
                let s = args.value()?.to_string();
                let v: RPC = serde_json::from_str(&s).unwrap();
                let mut hashmap = CALLBACKS.lock().unwrap();
                let f = hashmap.get_mut(&v.method).unwrap();
                let status = f(v.id, v.params);
                let x = IVector::<HString>::default();

                let js = match status {
                    0 => {
                        format!(
                            r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                            v.id, v.id
                        )
                    }
                    _ => {
                        format!(
                            r#"window._rpc[{}].reject("RPC call fail"); window._rpc[{}] = undefined"#,
                            v.id, v.id
                        )
                    }
                };
                // TODO send back execution result
                // x.append(HString::from(js));
                // wv.invoke_script_async("eval", x)?;
                Ok(())
            },
        ))?;

        RawWebView::init(w, "window.external.invoke = s => window.external.notify(s)")?;
        resize(&(*w).webview, (*w).window.hwnd() as *mut _);

        Ok(w)
    }

    pub unsafe fn navigate(webview: *mut RawWebView, url: &str) -> Result<(), Error> {
        (*webview).webview.navigate(Uri::create_uri(url)?)?;
        Ok(())
        // std::string html = html_from_uri(url);
        // if (html != "") {
        //   m_webview.NavigateToString(winrt::to_hstring(html));
        // } else {
        //   Uri uri(winrt::to_hstring(url));
        //   m_webview.Navigate(uri);
        // }
    }

    pub unsafe fn init(webview: *mut RawWebView, js: &str) -> Result<(), Error> {
        let script = String::from("(function(){") + js + "})();";
        (*webview).webview.add_initialize_script(script)?;
        Ok(())
    }

    pub unsafe fn eval(webview: *mut RawWebView, js: &str) -> Result<(), Error> {
        let x = IVector::<HString>::default();
        let _ = x.append(HString::from(js));
        (*webview).webview.invoke_script_async("name", x)?;
        Ok(())
    }

    pub unsafe fn bind<F>(webview: *mut RawWebView, name: &str, f: F) -> Result<(), Error>
    where
        F: FnMut(i8, Vec<String>) -> i32 + Sync + Send + 'static,
    {
        let js = format!(
            r#"var name = {:?};
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
            "#,
            name
        );
        (*webview).webview.add_initialize_script(js)?;
        CALLBACKS.lock().unwrap().insert(name.to_string(), Box::new(f));
        Ok(())
    }

    pub unsafe fn run(webview: *mut RawWebView) {
        let w = Box::from_raw(webview);
        let window = w.window;
        let webview = w.webview;
        w.events.run(move |event, _, control_flow| {
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

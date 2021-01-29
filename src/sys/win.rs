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

mod bindings {
    ::windows::include_bindings!();
}

use bindings::windows::foundation::collections::*;
use bindings::windows::foundation::*;
use bindings::windows::web::ui::*;
use bindings::windows::web::ui::interop::*;
use windows::{HString, Abi, RuntimeType};

static CALLBACKS: Lazy<Mutex<HashMap<String, Box<dyn FnMut(i8, Vec<String>) -> i32 + Sync + Send>>>> = Lazy::new(|| {
    let m = HashMap::new();
    Mutex::new(m)
});

#[derive(Debug, Serialize, Deserialize)]
struct RPC {
    id: i8,
    method: String,
    params: Vec<String>,
}

pub struct InnerWindow {
    events: Option<EventLoop<()>>,
    pub window: Window,
    webview: WebViewControl,
}

impl InnerWindow {
    pub fn new() -> Result<Self, Error> {
        let events = EventLoop::new();
        let window = Window::new(&events)?;

        // Webview
        let op = WebViewControlProcess::new()?
            .create_web_view_control_async(window.hwnd() as i64, Rect::default())?;

        unsafe {
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
        }

        let webview = op.get_results()?;
        webview.settings()?.set_is_script_notify_allowed(true)?;
        webview.set_is_visible(true)?;

        webview.script_notify(TypedEventHandler::new(
            |wv: &<IWebViewControl as RuntimeType>::DefaultType, args: &<WebViewControlScriptNotifyEventArgs as RuntimeType>::DefaultType| {
                if let Some(wv) = wv {
                    if let Some(args) = args {
                        let s = args.value()?.to_string();
                        let v: RPC = serde_json::from_str(&s).unwrap();
                        let mut hashmap = CALLBACKS.lock().unwrap();
                        let f = hashmap.get_mut(&v.method).unwrap();
                        let status = f(v.id, v.params);

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
                        let cstring = CString::new(js).unwrap();
                        let x: IVector<HString> = unsafe { IVector::from_abi(crate::ivector(cstring.as_ptr()))? };
                        wv.invoke_script_async("eval", x)?;
                    }
                }

                Ok(())
            },
        ))?;

        let w = InnerWindow {
            events: Some(events),
            window,
            webview,
        };

        // TODO NavigateToString/url as parameter
        w.webview.navigate(Uri::create_uri("https://google.com")?)?;

        w.init("window.external.invoke = s => window.external.notify(s)")?;
        w.resize();

        w.init("window.x = 42")?;
        w.eval("window.x")?;
        w.bind("xxx", |seq, req| {
            println!("The seq is: {}", seq);
            println!("The req is: {:?}", req);
            0
        })?;



        Ok(w)
    }

    pub fn init(&self, js: &str) -> Result<(), Error> {
        let script = String::from("(function(){") + js + "})();";
        //let s = HString::from(js);
        // self.webview.navigation_starting(TypedEventHandler::new(
        //     |wv: &<IWebViewControl as RuntimeType>::DefaultType, _| {
        //         if let Some(wv) = wv {
        //             wv.add_initialize_script(script)?;
        //         }
        //         Ok(())
        //     }
        // ));
        self.webview.add_initialize_script(script)?;
        Ok(())
    }

    pub fn eval(&self, js: &str) -> Result<(), Error> {
        let cstring = CString::new(js)?;
        let x: IVector<HString> = unsafe { IVector::from_abi(crate::ivector(cstring.as_ptr()))? };
        self.webview.invoke_script_async("eval", x)?;
        Ok(())
    }

    pub fn bind<F>(&self, name: &str, f: F) -> Result<(), Error>
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
        self.webview.add_initialize_script(js)?;
        CALLBACKS.lock().unwrap().insert(name.to_string(), Box::new(f));
        Ok(())
    }

    pub fn run(mut self) {
        if let Some(events) = self.events.take() {
            events.run(move |event, _, control_flow| {
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
                        self.resize();
                    }
                    _ => (),
                }
            });
        }
    }

    fn resize(&self) {
        let wnd = self.window.hwnd() as HWND;
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

            self.webview.set_bounds(r).unwrap();
        }
    }
}

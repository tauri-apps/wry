#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

use crate::Error;

use std::{
    collections::HashMap,
    ffi::CString,
    marker::{Send, Sync},
    ptr::{null, null_mut},
    sync::Mutex,
};

use once_cell::sync::Lazy;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::WindowExtWindows,
    window::Window,
};

use bindings::windows::{
    foundation::collections::*,
    foundation::*,
    web::ui::interop::*,
    web::ui::*,
    win32::{com::*, display_devices::*, system_services::*, windows_and_messaging::*},
};
use windows::{Abi, HString, RuntimeType, BOOL};

static CALLBACKS: Lazy<
    Mutex<HashMap<String, Box<dyn FnMut(i8, Vec<String>) -> i32 + Sync + Send>>>,
> = Lazy::new(|| {
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

        if op.status()? != AsyncStatus::Completed {
            // Safety: System API is unsafe
            let h = unsafe { CreateEventA(null_mut(), BOOL(0), BOOL(0), null()) };
            let mut hs = h.clone();
            op.set_completed(AsyncOperationCompletedHandler::new(move |_, _| {
                // Safety: System API is unsafe
                unsafe {
                    SetEvent(h);
                }
                Ok(())
            }))?;

            // Safety: System API is unsafe
            unsafe {
                CoWaitForMultipleHandles(
                    28, //COWAIT_DISPATCH_WINDOW_MESSAGES | COWAIT_DISPATCH_CALLS | COWAIT_INPUTAVAILABLE
                    INFINITE, 1, &mut hs.0, &mut 0u32,
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
                        // Safety: Create IVector from Winrt/C++
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
        self.webview.add_initialize_script(script)?;
        Ok(())
    }

    pub fn eval(&self, js: &str) -> Result<(), Error> {
        let cstring = CString::new(js)?;
        // Safety: Create IVector from Winrt/C++
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
        CALLBACKS
            .lock()
            .unwrap()
            .insert(name.to_string(), Box::new(f));
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
        let wnd = HWND(self.window.hwnd() as isize);

        if wnd.0 == 0 {
            return;
        }
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // Safety: System API is unsafe
        unsafe {
            GetClientRect(wnd, &mut r);
        }
        let r = Rect {
            x: r.left as f32,
            y: r.top as f32,
            width: (r.right - r.left) as f32,
            height: (r.bottom - r.top) as f32,
        };

        self.webview.set_bounds(r).unwrap();
    }
}

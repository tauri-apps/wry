#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

use crate::platform::{CALLBACKS, RPC};
use crate::{Dispatcher, Result};

use std::{
    ffi::CString,
    marker::{Send, Sync},
    os::raw::{c_char, c_void},
    ptr::{null, null_mut},
};

use bindings::windows::{
    foundation::collections::*,
    foundation::*,
    web::ui::interop::*,
    web::ui::*,
    win32::{com::*, display_devices::*, system_services::*, windows_and_messaging::*},
};
use windows::{Abi, HString, RuntimeType, BOOL};

#[cfg(target_os = "windows")]
extern "C" {
    fn ivector(js: *const c_char) -> *mut c_void;
}

pub struct InnerWebView {
    webview: WebViewControl,
    window_id: i64,
}

impl InnerWebView {
    pub fn new(hwnd: *mut c_void) -> Result<Self> {
        // Webview
        let op = WebViewControlProcess::new()?
            .create_web_view_control_async(hwnd as i64, Rect::default())?;

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

        let window_id = hwnd as i64;
        webview.script_notify(TypedEventHandler::new(
            move |wv: &<IWebViewControl as RuntimeType>::DefaultType, args: &<WebViewControlScriptNotifyEventArgs as RuntimeType>::DefaultType| {
                if let Some(wv) = wv {
                    if let Some(args) = args {
                        let s = args.value()?.to_string();
                        let v: RPC = serde_json::from_str(&s).unwrap();
                        let mut hashmap = CALLBACKS.lock().unwrap();
                        let (f, d) = hashmap.get_mut(&(window_id, v.method)).unwrap();
                        let status = f(d, v.id, v.params);

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
                        let x: IVector<HString> = unsafe { IVector::from_abi(ivector(cstring.as_ptr()))? };
                        wv.invoke_script_async("eval", x)?;
                    }
                }

                Ok(())
            },
        ))?;

        let w = InnerWebView {
            webview,
            window_id: hwnd as i64,
        };

        w.init("window.external.invoke = s => window.external.notify(s)")?;
        w.resize(hwnd);

        Ok(w)
    }

    pub fn init(&self, js: &str) -> Result<()> {
        let script = String::from("(function(){") + js + "})();";
        self.webview.add_initialize_script(script)?;
        Ok(())
    }

    pub fn add_callback<F>(&self, name: &str, f: F, dispatcher: Dispatcher)
    where
        F: FnMut(&Dispatcher, i32, Vec<String>) -> i32 + Send + 'static,
    {
        CALLBACKS.lock().unwrap().insert(
            (self.window_id, name.to_string()),
            (Box::new(f), dispatcher),
        );
    }

    pub fn eval(&self, js: &str) -> Result<()> {
        let cstring = CString::new(js)?;
        // Safety: Create IVector from Winrt/C++
        let x: IVector<HString> = unsafe { IVector::from_abi(ivector(cstring.as_ptr()))? };
        self.webview.invoke_script_async("eval", x)?;
        Ok(())
    }

    pub fn navigate(&self, url: &str) -> Result<()> {
        Ok(self.webview.navigate(Uri::create_uri(url)?)?)
    }

    pub fn navigate_to_string(&self, url: &str) -> Result<()> {
        Ok(self.webview.navigate_to_string(url)?)
    }

    pub fn resize(&self, hwnd: *mut c_void) {
        let wnd = HWND(hwnd as isize);

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

unsafe impl Send for InnerWebView {}
unsafe impl Sync for InnerWebView {}

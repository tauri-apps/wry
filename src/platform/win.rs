#[allow(dead_code)]
mod bindings {
    ::windows::include_bindings!();
}

use crate::platform::{CALLBACKS, RPC, win::windows::{foundation::IAsyncOperation, storage::streams::{IInputStream, IInputStreamReference}}};
use crate::Result;

use std::{
    ffi::CString,
    marker::{Send, Sync},
    os::raw::{c_char, c_void},
    ptr::{null, null_mut},
};
use bindings::windows;
// use bindings::windows::*;
// use bindings::windows::web::*;
use bindings::windows::{
    foundation::collections::*,
    foundation::*,
    web::ui::interop::*,
    web::ui::*,
    win32::{com::*, display_devices::*, system_services::*, windows_and_messaging::*},
    storage::streams::{FileRandomAccessStream},
    storage::{FileAccessMode, StorageFile}
};
use ::windows::{Abi, HString, RuntimeType, BOOL, implement, Param,};

#[cfg(target_os = "windows")]
extern "C" {
    fn ivector(js: *const c_char) -> *mut c_void;
}

pub struct InnerWebView {
    webview: WebViewControl,
}

#[implement(windows::foundation::IAsyncOperation)]
struct Other(String);
impl Other {
    fn get(&self) {
        self.0
    }
}
#[implement(windows::web::IUriToStreamResolver)]
struct MyResolver();

impl MyResolver {
            pub fn uri_to_stream_async<'a, T0__: Into<Param<'a, Uri>>>(
                &self,
                uri: T0__
            ) -> Result<IAsyncOperation<IInputStream>> {
                // let mut input: FileInputStream = {};
                let mut file: IInputStream = FileRandomAccessStream::open_async("test.html", FileAccessMode(0)).unwrap().into();
                let mut input: IInputStreamReference = IInputStreamReference::from(file.into());

                Ok(input.open_sequential_read_async().unwrap())
                // Ok(IAsyncOperation(file))
    }
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
                        let x: IVector<HString> = unsafe { IVector::from_abi(ivector(cstring.as_ptr()))? };
                        wv.invoke_script_async("eval", x)?;
                    }
                }

                Ok(())
            },
        ))?;

        let w = InnerWebView { webview };

        w.init("window.external.invoke = s => window.external.notify(s)")?;
        w.resize(hwnd);

        Ok(w)
    }

    pub fn init(&self, js: &str) -> Result<()> {
        let script = String::from("(function(){") + js + "})();";
        self.webview.add_initialize_script(script)?;
        Ok(())
    }

    pub fn register_buffer_protocol<F: 'static + Fn(&str) -> Vec<u8>>(&self, protocol: String, handler: F) -> Result<()> {
        // self.webview.uri_to_stream_async();
        // let test = IUriToStreamResolver::new();
        self.webview.web_resource_requested(TypedEventHandler::new(|arg, arg2| {
            // let mut res = HttpResponseMessage::new();
            // res.set_content();
            println!("{:?}", arg2);
            Ok(())
        }));
        Ok(())
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

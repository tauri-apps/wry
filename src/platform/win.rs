use crate::platform::{CALLBACKS, RPC};
use crate::{Dispatcher, Result};

use std::{
    ffi::CString,
    marker::Send,
    os::raw::{c_char, c_void},
    ptr::{null, null_mut},
    rc::Rc,
};

use once_cell::unsync::OnceCell;
use webview2::{Controller, WebView};
use winapi::shared::windef::HWND;
use winapi::um::winuser::GetClientRect;

pub struct InnerWebView {
    pub controller: Rc<OnceCell<Controller>>,
    hwnd: HWND,
    initialization_scripts: Vec<String>,
    url: Option<String>,
    window_id: i64,
}

impl InnerWebView {
    pub fn new(hwnd: *mut c_void) -> Result<Self> {
        let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
        Ok(Self{
            controller,
            hwnd: hwnd as HWND,
            initialization_scripts: vec![],
            url: None,
            window_id: 0,
        })
    }

    pub fn build(&mut self) -> Result<()> {
        let url = self.url.take();
        let mut scripts = vec![];
        std::mem::swap(&mut self.initialization_scripts, &mut scripts);
        let hwnd = self.hwnd;
        let controller_clone = self.controller.clone();

        webview2::EnvironmentBuilder::new().build(move |env| {
            env?.create_controller(hwnd, move |controller| {
                let controller = controller?;
                let w = controller.get_webview()?;

                let _ = w.get_settings().map(|settings| {
                    let _ = settings.put_is_status_bar_enabled(false);
                    let _ = settings.put_are_default_context_menus_enabled(true);
                    let _ = settings.put_are_dev_tools_enabled(true);
                    let _ = settings.put_is_zoom_control_enabled(false);
                });

                unsafe {
                    let mut rect = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    controller.put_bounds(rect)?;
                }

                for js in scripts {
                    w.add_script_to_execute_on_document_created(&js, |_|(Ok(())));
                }
                w.add_script_to_execute_on_document_created(
                    "window.external = {
                      invoke: function(s) {
                        window.webkit.messageHandlers.external.postMessage(s);
                      },
                    };",
                    |_|(Ok(()))
                )?;

                w.add_web_message_received(|webview, args| {
                    let string = args.get_web_message_as_json()?;
                    dbg!(string);
                    Ok(())
                })?;
                // w.add_permission_requested(|webview, args| {
                //     todo!()
                // });

                if let Some(url) = url {
                    w.navigate(&url)?;
                }

                let _ = controller_clone.set(controller);
                Ok(())
            })
        })?;

        Ok(())
    }

    pub fn init(&mut self, js: &str) -> Result<()> {
        self.initialization_scripts.push(js.to_string());
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
        if let Some(c) = self.controller.get() {
            let webview = c.get_webview()?;
            webview.execute_script(js, |_|(Ok(())))?;
        }
        Ok(())
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.url = Some(url.to_string());
        Ok(())
    }

    pub fn navigate_to_string(&self, url: &str) -> Result<()> {
        todo!()
    }

    pub fn resize(&self) {
        todo!()
    }
}

use crate::platform::{CALLBACKS, RPC};
use crate::{Dispatcher, Result};

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    marker::Send,
    os::raw::c_void,
    rc::Rc,
};

use once_cell::unsync::OnceCell;
use webview2::{Controller, PermissionKind, PermissionState};
use winapi::{shared::windef::HWND, um::winuser::GetClientRect};
use winit::{platform::windows::WindowExtWindows, window::Window};

pub struct InnerWebView {
    controller: Rc<OnceCell<Controller>>,
    debug: bool,
    hwnd: HWND,
    initialization_scripts: Vec<String>,
    url: Option<(String, bool)>,
    window_id: i64,
}

impl InnerWebView {
    pub fn new(window: &Window, debug: bool) -> Result<Self> {
        let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
        let mut hasher = DefaultHasher::new();
        window.id().hash(&mut hasher);
        let window_id = hasher.finish() as i64;
        Ok(Self {
            controller,
            debug,
            hwnd: window.hwnd() as HWND,
            initialization_scripts: vec![],
            url: None,
            window_id,
        })
    }

    pub fn build(&mut self) -> Result<()> {
        let debug = self.debug;
        let url = self.url.take();
        let mut scripts = vec![];
        std::mem::swap(&mut self.initialization_scripts, &mut scripts);
        let hwnd = self.hwnd;
        let controller_clone = self.controller.clone();

        let window_id = self.window_id;
        webview2::EnvironmentBuilder::new().build(move |env| {
            env?.create_controller(hwnd, move |controller| {
                let controller = controller?;
                let w = controller.get_webview()?;

                let settings = w.get_settings()?;
                settings.put_is_status_bar_enabled(false)?;
                settings.put_are_default_context_menus_enabled(true)?;
                settings.put_is_zoom_control_enabled(false)?;
                if debug {
                    settings.put_are_dev_tools_enabled(true)?;
                }

                unsafe {
                    let mut rect = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    controller.put_bounds(rect)?;
                }

                for js in scripts {
                    w.add_script_to_execute_on_document_created(&js, |_|(Ok(())))?;
                }
                w.add_script_to_execute_on_document_created(
                    "window.external={invoke:s=>window.chrome.webview.postMessage(s)}",
                    |_|(Ok(()))
                )?;

                w.add_web_message_received(move |webview, args| {
                    let s = args.try_get_web_message_as_string()?;
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
                    webview.execute_script(&js, |_|(Ok(())))?;
                    Ok(())
                })?;
                w.add_permission_requested(|_, args| {
                    let kind = args.get_permission_kind()?;
                    if kind == PermissionKind::ClipboardRead {
                        args.put_state(PermissionState::Allow)?;
                    }
                    Ok(())
                })?;

                if let Some(url) = url {
                    if url.1 {
                        w.navigate(&url.0)?;
                    } else {
                        w.navigate_to_string(&url.0)?;
                    }

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
            webview.execute_script(js, |_| (Ok(())))?;
        }
        Ok(())
    }

    pub fn navigate(&mut self, url: &str) -> Result<()> {
        self.url = Some((url.to_string(), true));
        Ok(())
    }

    pub fn navigate_to_string(&mut self, url: &str) -> Result<()> {
        self.url = Some((url.to_string(), false));
        Ok(())
    }

    pub fn resize(&self, hwnd: *mut c_void) -> Result<()> {
        let hwnd = hwnd as HWND;

        unsafe {
            let mut rect = std::mem::zeroed();
            GetClientRect(hwnd, &mut rect);
            if let Some(c) = self.controller.get() {
                c.put_bounds(rect)?;
            }
        }

        Ok(())
    }
}

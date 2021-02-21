use crate::platform::{CALLBACKS, RPC};
use crate::webview::WV;
use crate::Result;

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    os::raw::c_void,
    rc::Rc,
};

use once_cell::unsync::OnceCell;
use url::Url;
use webview2::{Controller, PermissionKind, PermissionState};
use winapi::{shared::windef::HWND, um::winuser::GetClientRect};
use winit::{platform::windows::WindowExtWindows, window::Window};

pub struct InnerWebView {
    controller: Rc<OnceCell<Controller>>,
}

impl WV for InnerWebView {
    type Window = Window;

    fn new(
        window: &Window,
        debug: bool,
        scripts: Vec<String>,
        url: Option<Url>,
        transparent: bool,
    ) -> Result<Self> {
        let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
        let mut hasher = DefaultHasher::new();
        window.id().hash(&mut hasher);
        let window_id = hasher.finish() as i64;
        let hwnd = window.hwnd() as HWND;
        let controller_clone = controller.clone();

        // Webview controller
        webview2::EnvironmentBuilder::new().build(move |env| {
            env?.create_controller(hwnd, move |controller| {
                let controller = controller?;
                let w = controller.get_webview()?;

                // Enable sensible defaults
                let settings = w.get_settings()?;
                settings.put_is_status_bar_enabled(false)?;
                settings.put_are_default_context_menus_enabled(true)?;
                settings.put_is_zoom_control_enabled(false)?;
                if debug {
                    settings.put_are_dev_tools_enabled(true)?;
                }

                // Safety: System calls are unsafe
                unsafe {
                    let mut rect = std::mem::zeroed();
                    GetClientRect(hwnd, &mut rect);
                    controller.put_bounds(rect)?;
                }

                // Initialize scripts
                for js in scripts {
                    w.add_script_to_execute_on_document_created(&js, |_| (Ok(())))?;
                }
                w.add_script_to_execute_on_document_created(
                    "window.external={invoke:s=>window.chrome.webview.postMessage(s)}",
                    |_| (Ok(()))
                )?;

                // Message handler
                w.add_web_message_received(move |webview, args| {
                    let s = args.try_get_web_message_as_string()?;
                    let v: RPC = serde_json::from_str(&s).unwrap();
                    let mut hashmap = CALLBACKS.lock().unwrap();
                    let (f, d) = hashmap.get_mut(&(window_id, v.method)).unwrap();
                    let status = f(d, v.id, v.params);

                    let js = match status {
                        Ok(()) => {
                            format!(
                                r#"window._rpc[{}].resolve("RPC call success"); window._rpc[{}] = undefined"#,
                                v.id, v.id
                            )
                        }
                        Err(e) => {
                            format!(
                                r#"window._rpc[{}].reject("RPC call fail with error {}"); window._rpc[{}] = undefined"#,
                                v.id, e, v.id
                            )
                        }
                    };

                    webview.execute_script(&js, |_| (Ok(())))?;
                    Ok(())
                })?;

                // Enable clipboard
                w.add_permission_requested(|_, args| {
                    let kind = args.get_permission_kind()?;
                    if kind == PermissionKind::ClipboardRead {
                        args.put_state(PermissionState::Allow)?;
                    }
                    Ok(())
                })?;

                // Navigation
                if let Some(url) = url {
                    if url.cannot_be_a_base() {
                        w.navigate_to_string(url.as_str())?;
                    } else {
                        w.navigate(url.as_str())?;
                    }
                }

                let _ = controller_clone.set(controller);
                Ok(())
            })
        })?;

        Ok(Self { controller })
    }

    fn eval(&self, js: &str) -> Result<()> {
        if let Some(c) = self.controller.get() {
            let webview = c.get_webview()?;
            webview.execute_script(js, |_| (Ok(())))?;
        }
        Ok(())
    }
}

impl InnerWebView {
    pub fn resize(&self, hwnd: *mut c_void) -> Result<()> {
        let hwnd = hwnd as HWND;

        // Safety: System calls are unsafe
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

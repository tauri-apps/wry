// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use crate::{
  application::platform::windows::EventLoopExtWindows,
  webview::{WebContext, WebViewAttributes},
  Result,
};

use file_drop::FileDropController;

use std::{collections::HashSet, os::raw::c_void, rc::Rc};

use once_cell::unsync::OnceCell;
use webview2::{Controller, PermissionKind, PermissionState, WebView};
use winapi::{
  shared::{windef::HWND, winerror::E_FAIL},
  um::winuser::{DestroyWindow, GetClientRect},
};

use crate::application::{
  event_loop::{ControlFlow, EventLoop},
  platform::{run_return::EventLoopExtRunReturn, windows::WindowExtWindows},
  window::Window,
};

pub struct InnerWebView {
  pub(crate) controller: Rc<OnceCell<Controller>>,
  webview: Rc<OnceCell<WebView>>,
  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  file_drop_controller: Rc<OnceCell<FileDropController>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    mut attributes: WebViewAttributes,
    web_context: Option<&WebContext>,
  ) -> Result<Self> {
    let hwnd = window.hwnd() as HWND;

    let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
    let controller_clone = controller.clone();

    let file_drop_controller: Rc<OnceCell<FileDropController>> = Rc::new(OnceCell::new());
    let file_drop_controller_clone = file_drop_controller.clone();

    let mut webview_builder = webview2::EnvironmentBuilder::new();

    if let Some(web_context) = web_context {
      if let Some(data_directory) = web_context.data_directory() {
        webview_builder = webview_builder.with_user_data_folder(&data_directory);
      }
    }

    // Webview controller
    webview_builder.build(move |env| {
      let env = env?;
      let env_ = env.clone();
      env.create_controller(hwnd, move |controller| {
        let controller = controller?;
        let w = controller.get_webview()?;

        w.add_window_close_requested(move |_| {
          if unsafe { DestroyWindow(hwnd as HWND) } != 0 {
            Ok(())
          } else {
            Err(webview2::Error::new(E_FAIL))
          }
        })?;

        // Transparent
        if attributes.transparent {
          if let Ok(c2) = controller.get_controller2() {
            c2.put_default_background_color(webview2_sys::Color {
              r: 0,
              g: 0,
              b: 0,
              a: 0,
            })?;
          }
        }

        // Enable sensible defaults
        let settings = w.get_settings()?;
        settings.put_is_status_bar_enabled(false)?;
        settings.put_are_default_context_menus_enabled(true)?;
        settings.put_is_zoom_control_enabled(false)?;
        settings.put_are_dev_tools_enabled(false)?;
        debug_assert_eq!(settings.put_are_dev_tools_enabled(true)?, ());

        // Safety: System calls are unsafe
        unsafe {
          let mut rect = std::mem::zeroed();
          GetClientRect(hwnd, &mut rect);
          controller.put_bounds(rect)?;
        }

        // Initialize scripts
        w.add_script_to_execute_on_document_created(
          r#"
            window.external={invoke:s=>window.chrome.webview.postMessage(s)};

            window.addEventListener('mousedown', (e) => {
              if (e.buttons === 1) window.chrome.webview.postMessage('__WEBVIEW_LEFT_MOUSE_DOWN__')
            });
            window.addEventListener('mousemove', () => window.chrome.webview.postMessage('__WEBVIEW_MOUSE_MOVE__'));
          "#,
          |_| (Ok(())),
        )?;
        for js in attributes.initialization_scripts {
          w.add_script_to_execute_on_document_created(&js, |_| (Ok(())))?;
        }

        // Message handler
        let window_ = window.clone();

        let rpc_handler = attributes.rpc_handler.take();
        w.add_web_message_received(move |webview, args| {
          let js = args.try_get_web_message_as_string()?;
          if js == "__WEBVIEW_LEFT_MOUSE_DOWN__" || js == "__WEBVIEW_MOUSE_MOVE__" {
            if !window_.is_decorated() && window_.is_resizable() {
              use winapi::um::winuser::{
                HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTLEFT, HTRIGHT,
                HTTOP, HTTOPLEFT, HTTOPRIGHT, HTCLIENT, GetCursorPos,
              };
              use crate::application::{window::CursorIcon,platform::windows::hit_test};

              let (cx, cy);
              unsafe {
                let mut point = std::mem::zeroed();
                GetCursorPos(&mut point);
                cx = point.x;
                cy = point.y;
              };
              let result = hit_test(window_.hwnd() as _, cx, cy);
              let cursor = match result {
                    HTLEFT => CursorIcon::WResize,
                    HTTOP => CursorIcon::NResize,
                    HTRIGHT => CursorIcon::EResize,
                    HTBOTTOM => CursorIcon::SResize,
                    HTTOPLEFT => CursorIcon::NwResize,
                    HTTOPRIGHT => CursorIcon::NeResize,
                    HTBOTTOMLEFT => CursorIcon::SwResize,
                    HTBOTTOMRIGHT => CursorIcon::SeResize,
                    _ => CursorIcon::Arrow,
              };
              window_.set_cursor_icon(cursor);

              if js == "__WEBVIEW_LEFT_MOUSE_DOWN__"  {
                // this check is necessary, otherwise any window dragging implementation won't work
                if result != HTCLIENT {
                  window_.begin_resize_drag(result);
                }
              }
            }
            // these are internal messages, rpc_handlers don't need it so exit early
            return Ok(());
          }

          if let Some(rpc_handler) = &rpc_handler {
            match super::rpc_proxy(&window_, js, rpc_handler) {
              Ok(result) => {
                if let Some(ref script) = result {
                  webview.execute_script(script, |_| (Ok(())))?;
                }
              }
              Err(e) => {
                eprintln!("{}", e);
              }
            }
          }
          Ok(())
        })?;

        let mut custom_protocol_names = HashSet::new();
        if !attributes.custom_protocols.is_empty() {
          for (name, _) in &attributes.custom_protocols {
            // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
            // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
            custom_protocol_names.insert(name.clone());
            w.add_web_resource_requested_filter(
              &format!("https://custom.protocol.{}_*", name),
              webview2::WebResourceContext::All,
            )?;
          }

          let custom_protocols = attributes.custom_protocols;
          let env_clone = env_.clone();
          let window_ = window.clone();
          w.add_web_resource_requested(move |_, args| {
            let uri = args.get_request()?.get_uri()?;
            // Undo the protocol workaround when giving path to resolver
            let path = uri
              .replace("https://custom.protocol.", "")
              .replacen("_", "://", 1);
            let scheme = path.split("://").next().unwrap();

            match (custom_protocols
              .iter()
              .find(|(name, _)| name == &scheme)
              .unwrap()
              .1)(&window_, &path)
            {
              Ok((content, mime)) => {
                let stream = webview2::Stream::from_bytes(&content);
                let response = env_clone.create_web_resource_response(
                  stream,
                  200,
                  "OK",
                  &format!("Content-Type: {}", mime),
                )?;
                args.put_response(response)?;
                Ok(())
              }
              Err(_) => Err(webview2::Error::from(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Error loading requested file",
              ))),
            }
          })?;
        }

        // Enable clipboard
        w.add_permission_requested(|_, args| {
          let kind = args.get_permission_kind()?;
          if kind == PermissionKind::ClipboardRead {
            args.put_state(PermissionState::Allow)?;
          }
          Ok(())
        })?;

        // Navigation
        if let Some(url) = attributes.url {
          if url.cannot_be_a_base() {
            let s = url.as_str();
            if let Some(pos) = s.find(',') {
              let (_, path) = s.split_at(pos + 1);
              w.navigate_to_string(path)?;
            }
          } else {
            let mut url_string = String::from(url.as_str());
            let name = url.scheme();
            if custom_protocol_names.contains(name) {
              // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
              // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
              url_string = url.as_str().replace(
                &format!("{}://", name),
                &format!("https://custom.protocol.{}_", name),
              )
            }
            w.navigate(&url_string)?;
          }
        }

        controller.put_is_visible(true)?;
        controller.move_focus(webview2::MoveFocusReason::Programmatic)?;
        let _ = controller_clone.set(controller);

        if let Some(file_drop_handler) = attributes.file_drop_handler {
          let mut file_drop_controller = FileDropController::new();
          file_drop_controller.listen(hwnd, window.clone(), file_drop_handler);
          let _ = file_drop_controller_clone.set(file_drop_controller);
        }

        Ok(())
      })
    })?;

    // Wait until webview is actually created
    let mut event_loop: EventLoop<()> = EventLoop::new_any_thread();
    let controller_clone = controller.clone();
    let webview: Rc<OnceCell<WebView>> = Rc::new(OnceCell::new());
    let webview_clone = webview.clone();
    event_loop.run_return(|_, _, control_flow| {
      if let Some(c) = controller_clone.get() {
        if let Ok(wv) = c.get_webview() {
          *control_flow = ControlFlow::Exit;
          let _ = webview_clone.set(wv);
        } else {
          *control_flow = ControlFlow::Poll;
        }
      }
    });
    // TODO: OnceCell into_inner for controller

    Ok(Self {
      controller,
      webview,
      file_drop_controller,
    })
  }

  pub fn print(&self) {
    let _ = self.eval("window.print()");
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    if let Some(w) = self.webview.get() {
      w.execute_script(js, |_| (Ok(())))?;
    }
    Ok(())
  }

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

  pub fn focus(&self) {
    if let Some(c) = self.controller.get() {
      let _ = c.move_focus(webview2::MoveFocusReason::Programmatic);
    }
  }
}

pub fn platform_webview_version() -> Result<String> {
  let webview_builder = webview2::EnvironmentBuilder::new();
  let version = webview_builder
    .get_available_browser_version_string()
    .expect("Unable to get webview2 version");
  Ok(version)
}

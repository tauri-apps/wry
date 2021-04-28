// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

mod file_drop;

use crate::{
  webview::{mimetype::MimeType, FileDropEvent, RpcRequest, RpcResponse},
  Result,
};

use file_drop::FileDropController;

use std::{collections::HashSet, os::raw::c_void, path::PathBuf, rc::Rc};

use once_cell::unsync::OnceCell;
use url::Url;
use webview2::{Controller, PermissionKind, PermissionState};
use winapi::{shared::windef::HWND, um::winuser::GetClientRect};

use crate::application::{platform::windows::WindowExtWindows, window::Window};

pub struct InnerWebView {
  controller: Rc<OnceCell<Controller>>,

  // Store FileDropController in here to make sure it gets dropped when
  // the webview gets dropped, otherwise we'll have a memory leak
  #[allow(dead_code)]
  file_drop_controller: Rc<OnceCell<FileDropController>>,
}

impl InnerWebView {
  pub fn new(
    window: Rc<Window>,
    scripts: Vec<String>,
    url: Option<Url>,
    transparent: bool,
    custom_protocols: Vec<(
      String,
      Box<dyn Fn(&Window, &str) -> Result<Vec<u8>> + 'static>,
    )>,
    rpc_handler: Option<Box<dyn Fn(&Window, RpcRequest) -> Option<RpcResponse>>>,
    file_drop_handler: Option<Box<dyn Fn(&Window, FileDropEvent) -> bool>>,
    data_directory: Option<PathBuf>,
  ) -> Result<Self> {
    let hwnd = window.hwnd() as HWND;

    let controller: Rc<OnceCell<Controller>> = Rc::new(OnceCell::new());
    let controller_clone = controller.clone();

    let file_drop_controller: Rc<OnceCell<FileDropController>> = Rc::new(OnceCell::new());
    let file_drop_controller_clone = file_drop_controller.clone();

    let webview_builder: webview2::EnvironmentBuilder;
    let data_directory_provided: PathBuf;

    if let Some(data_directory) = data_directory {
      data_directory_provided = data_directory;
      webview_builder =
        webview2::EnvironmentBuilder::new().with_user_data_folder(&data_directory_provided);
    } else {
      webview_builder = webview2::EnvironmentBuilder::new();
    }

    // Webview controller
    webview_builder.build(move |env| {
      let env = env?;
      let env_ = env.clone();
      env.create_controller(hwnd, move |controller| {
        let controller = controller?;
        let w = controller.get_webview()?;

        // Transparent
        if transparent {
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
          "window.external={invoke:s=>window.chrome.webview.postMessage(s)}",
          |_| (Ok(())),
        )?;
        for js in scripts {
          w.add_script_to_execute_on_document_created(&js, |_| (Ok(())))?;
        }

        // Message handler
        let window_ = window.clone();
        w.add_web_message_received(move |webview, args| {
          let js = args.try_get_web_message_as_string()?;
          if let Some(rpc_handler) = rpc_handler.as_ref() {
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
        for (name, function) in custom_protocols {
          // WebView2 doesn't support non-standard protocols yet, so we have to use this workaround
          // See https://github.com/MicrosoftEdge/WebView2Feedback/issues/73
          custom_protocol_names.insert(name.clone());
          w.add_web_resource_requested_filter(
            &format!("https://custom-protocol-{}*", name),
            webview2::WebResourceContext::All,
          )?;
          let env_clone = env_.clone();
          let window_ = window.clone();
          w.add_web_resource_requested(move |_, args| {
            let uri = args.get_request()?.get_uri()?;
            // Undo the protocol workaround when giving path to resolver
            let path = &uri.replace(
              &format!("https://custom-protocol-{}", name),
              &format!("{}://", name),
            );

            match function(&window_, path) {
              Ok(content) => {
                let mime = MimeType::parse(&content, &uri);
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
        if let Some(url) = url {
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
                &format!("https://custom-protocol-{}", name),
              )
            }
            w.navigate(&url_string)?;
          }
        }

        controller.put_is_visible(true)?;
        let _ = controller_clone.set(controller);

        if let Some(file_drop_handler) = file_drop_handler {
          let mut file_drop_controller = FileDropController::new();
          file_drop_controller.listen(hwnd, window.clone(), file_drop_handler);
          let _ = file_drop_controller_clone.set(file_drop_controller);
        }

        Ok(())
      })
    })?;

    Ok(Self {
      controller,

      file_drop_controller,
    })
  }

  pub fn eval(&self, js: &str) -> Result<()> {
    if let Some(c) = self.controller.get() {
      let webview = c.get_webview()?;
      webview.execute_script(js, |_| (Ok(())))?;
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
}

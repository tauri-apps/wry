// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use dpi::{LogicalPosition, LogicalSize};
use winit::{
  application::ApplicationHandler,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
  window::{Window, WindowId},
};
use wry::{http::Request, Rect, WebView, WebViewBuilder};

enum UserEvent {
  CloseWindow(WindowId),
  NewTitle(WindowId, String),
  NewWindow,
}

struct App {
  webviews: HashMap<WindowId, (Window, WebView)>,
  proxy: EventLoopProxy<UserEvent>,
}

impl ApplicationHandler<UserEvent> for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let (window, webview) = create_new_window(
      format!("Window {}", self.webviews.len() + 1),
      event_loop,
      self.proxy.clone(),
    );
    self.webviews.insert(window.id(), (window, webview));
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    window_id: WindowId,
    event: WindowEvent,
  ) {
    match event {
      WindowEvent::Resized(size) => {
        if let Some((window, webview)) = self.webviews.get(&window_id) {
          let size = size.to_logical::<u32>(window.scale_factor());
          webview
            .set_bounds(Rect {
              position: LogicalPosition::new(0, 0).into(),
              size: LogicalSize::new(size.width, size.height).into(),
            })
            .unwrap();
        }
      }
      WindowEvent::CloseRequested => {
        self.webviews.remove(&window_id);
        if self.webviews.is_empty() {
          event_loop.exit();
        }
      }
      _ => {}
    }
  }

  fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
    match event {
      UserEvent::NewWindow => {
        let (window, webview) = create_new_window(
          format!("Window {}", self.webviews.len() + 1),
          event_loop,
          self.proxy.clone(),
        );
        self.webviews.insert(window.id(), (window, webview));
      }
      UserEvent::CloseWindow(id) => {
        self.webviews.remove(&id);
        if self.webviews.is_empty() {
          event_loop.exit();
        }
      }
      UserEvent::NewTitle(id, title) => {
        if let Some((window, _)) = self.webviews.get(&id) {
          window.set_title(&title);
        }
      }
    }
  }

  fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    ))]
    while gtk4::glib::MainContext::default().iteration(false) {}
  }
}

fn create_new_window(
  title: String,
  event_loop: &ActiveEventLoop,
  proxy: EventLoopProxy<UserEvent>,
) -> (Window, WebView) {
  let window = event_loop
    .create_window(Window::default_attributes().with_title(title))
    .unwrap();
  let window_id = window.id();

  let handler = move |req: Request<String>| {
    let body = req.body();
    match body.as_str() {
      "new-window" => {
        let _ = proxy.send_event(UserEvent::NewWindow);
      }
      "close" => {
        let _ = proxy.send_event(UserEvent::CloseWindow(window_id));
      }
      _ if body.starts_with("change-title") => {
        let title = body.replace("change-title:", "");
        let _ = proxy.send_event(UserEvent::NewTitle(window_id, title));
      }
      _ => {}
    }
  };

  let webview = WebViewBuilder::new()
    .with_html(
      r#"
        <button onclick="window.ipc.postMessage('new-window')">Open a new window</button>
        <button onclick="window.ipc.postMessage('close')">Close current window</button>
        <input oninput="window.ipc.postMessage(`change-title:${this.value}`)" />
    "#,
    )
    .with_ipc_handler(handler)
    .build_as_child(&window)
    .unwrap();

  (window, webview)
}

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  {
    gtk4::init().unwrap();

    #[cfg(feature = "x11")]
    winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
      let error = error as *mut x11_dl::xlib::XErrorEvent;
      (unsafe { (*error).error_code }) == 170
    }));
  }

  let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
  let proxy = event_loop.create_proxy();
  let mut app = App {
    webviews: HashMap::new(),
    proxy,
  };
  event_loop.run_app(&mut app).unwrap();
  Ok(())
}

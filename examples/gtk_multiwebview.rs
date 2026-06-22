// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use dpi::{LogicalPosition, LogicalSize};
use winit::{
  application::ApplicationHandler,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, EventLoop},
  window::{Window, WindowId},
};
use wry::{Rect, WebViewBuilder};

struct App {
  window: Option<Window>,
  webviews: Option<[wry::WebView; 4]>,
}

impl Default for App {
  fn default() -> Self {
    App {
      window: None,
      webviews: None,
    }
  }
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let window = event_loop
      .create_window(Window::default_attributes())
      .unwrap();

    let size = window.inner_size().to_logical::<u32>(window.scale_factor());

    let webview = WebViewBuilder::new()
      .with_bounds(Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: LogicalSize::new(size.width / 2, size.height / 2).into(),
      })
      .with_url("https://tauri.app")
      .build_as_child(&window)
      .unwrap();

    let webview2 = WebViewBuilder::new()
      .with_bounds(Rect {
        position: LogicalPosition::new(size.width / 2, 0).into(),
        size: LogicalSize::new(size.width / 2, size.height / 2).into(),
      })
      .with_url("https://github.com/tauri-apps/wry")
      .build_as_child(&window)
      .unwrap();

    let webview3 = WebViewBuilder::new()
      .with_bounds(Rect {
        position: LogicalPosition::new(0, size.height / 2).into(),
        size: LogicalSize::new(size.width / 2, size.height / 2).into(),
      })
      .with_url("https://twitter.com/TauriApps")
      .build_as_child(&window)
      .unwrap();

    let webview4 = WebViewBuilder::new()
      .with_bounds(Rect {
        position: LogicalPosition::new(size.width / 2, size.height / 2).into(),
        size: LogicalSize::new(size.width / 2, size.height / 2).into(),
      })
      .with_url("https://google.com")
      .build_as_child(&window)
      .unwrap();

    self.window = Some(window);
    self.webviews = Some([webview, webview2, webview3, webview4]);
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _window_id: WindowId,
    event: WindowEvent,
  ) {
    match event {
      WindowEvent::Resized(size) => {
        let window = self.window.as_ref().unwrap();
        let webviews = self.webviews.as_ref().unwrap();
        let size = size.to_logical::<u32>(window.scale_factor());

        webviews[0]
          .set_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webviews[1]
          .set_bounds(Rect {
            position: LogicalPosition::new(size.width / 2, 0).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webviews[2]
          .set_bounds(Rect {
            position: LogicalPosition::new(0, size.height / 2).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webviews[3]
          .set_bounds(Rect {
            position: LogicalPosition::new(size.width / 2, size.height / 2).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
      }
      WindowEvent::CloseRequested => event_loop.exit(),
      _ => {}
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

  let event_loop = EventLoop::new().unwrap();
  let mut app = App::default();
  event_loop.run_app(&mut app).unwrap();
  Ok(())
}

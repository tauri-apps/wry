// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  return linux_main();

  #[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  )))]
  return non_linux_main();
}

/// Linux / BSD: GTK4-native, works on both X11 and Wayland.
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
fn linux_main() -> wry::Result<()> {
  use std::cell::RefCell;

  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Multi WebView (GTK4 / Wayland)"));
    window.set_default_size(1200, 800);

    // 2×2 grid via nested boxes:
    //   vbox
    //   ├── top_row    (hbox: wv1 | wv2)
    //   └── bottom_row (hbox: wv3 | wv4)
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.set_homogeneous(true);
    window.set_child(Some(&vbox));

    let top_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    top_row.set_vexpand(true);
    top_row.set_homogeneous(true);
    vbox.append(&top_row);

    let bottom_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    bottom_row.set_vexpand(true);
    bottom_row.set_homogeneous(true);
    vbox.append(&bottom_row);

    window.present();

    let wv1 = wry::WebViewBuilder::new()
      .with_url("https://tauri.app")
      .build_gtk(&top_row)
      .unwrap();

    let wv2 = wry::WebViewBuilder::new()
      .with_url("https://github.com/tauri-apps/wry")
      .build_gtk(&top_row)
      .unwrap();

    let wv3 = wry::WebViewBuilder::new()
      .with_url("https://tauri.app/blog")
      .build_gtk(&bottom_row)
      .unwrap();

    let wv4 = wry::WebViewBuilder::new()
      .with_url("https://github.com/tauri-apps/tauri")
      .build_gtk(&bottom_row)
      .unwrap();

    // Keep webviews alive for the window's lifetime.
    let webviews = RefCell::new(Some([wv1, wv2, wv3, wv4]));
    window.connect_close_request(move |_| {
      webviews.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

/// macOS / Windows: winit + build_as_child.
#[cfg(not(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
)))]
fn non_linux_main() -> wry::Result<()> {
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
        .create_window(Window::default_attributes().with_title("Multi WebView"))
        .unwrap();

      let size = window.inner_size().to_logical::<u32>(window.scale_factor());

      let wv1 = WebViewBuilder::new()
        .with_bounds(Rect {
          position: LogicalPosition::new(0, 0).into(),
          size: LogicalSize::new(size.width / 2, size.height / 2).into(),
        })
        .with_url("https://tauri.app")
        .build_as_child(&window)
        .unwrap();

      let wv2 = WebViewBuilder::new()
        .with_bounds(Rect {
          position: LogicalPosition::new(size.width / 2, 0).into(),
          size: LogicalSize::new(size.width / 2, size.height / 2).into(),
        })
        .with_url("https://github.com/tauri-apps/wry")
        .build_as_child(&window)
        .unwrap();

      let wv3 = WebViewBuilder::new()
        .with_bounds(Rect {
          position: LogicalPosition::new(0, size.height / 2).into(),
          size: LogicalSize::new(size.width / 2, size.height / 2).into(),
        })
        .with_url("https://tauri.app/blog")
        .build_as_child(&window)
        .unwrap();

      let wv4 = WebViewBuilder::new()
        .with_bounds(Rect {
          position: LogicalPosition::new(size.width / 2, size.height / 2).into(),
          size: LogicalSize::new(size.width / 2, size.height / 2).into(),
        })
        .with_url("https://github.com/tauri-apps/tauri")
        .build_as_child(&window)
        .unwrap();

      self.window = Some(window);
      self.webviews = Some([wv1, wv2, wv3, wv4]);
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
  }

  let event_loop = EventLoop::new().unwrap();
  let mut app = App::default();
  event_loop.run_app(&mut app).unwrap();
  Ok(())
}

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
    window.set_title(Some("Cookies (GTK4 / Wayland)"));
    window.set_default_size(800, 600);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    let webview = wry::WebViewBuilder::new()
      .with_url("https://www.httpbin.org/cookies/set?foo=bar")
      .build_gtk(&vbox)
      .unwrap();

    webview
      .set_cookie(
        cookie::Cookie::build(("foo1", "bar1"))
          .domain("www.httpbin.org")
          .path("/")
          .secure(true)
          .http_only(true)
          .max_age(cookie::time::Duration::seconds(10))
          .inner(),
      )
      .unwrap();

    let cookie_deleted = cookie::Cookie::build(("will_be_deleted", "will_be_deleted"));
    webview.set_cookie(cookie_deleted.inner()).unwrap();

    println!("Setting Cookies:");
    for cookie in webview.cookies().unwrap() {
      println!("\t{cookie}");
    }

    println!("After Deleting:");
    webview.delete_cookie(cookie_deleted.inner()).unwrap();
    for cookie in webview.cookies().unwrap() {
      println!("\t{cookie}");
    }

    let webview = RefCell::new(Some(webview));
    window.connect_close_request(move |_| {
      webview.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

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

  #[derive(Default)]
  struct App {
    window: Option<Window>,
    webview: Option<wry::WebView>,
  }

  impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
      let window = event_loop
        .create_window(Window::default_attributes().with_title("Cookies"))
        .unwrap();

      let webview = WebViewBuilder::new()
        .with_url("https://www.httpbin.org/cookies/set?foo=bar")
        .build_as_child(&window)
        .unwrap();

      webview
        .set_cookie(
          cookie::Cookie::build(("foo1", "bar1"))
            .domain("www.httpbin.org")
            .path("/")
            .secure(true)
            .http_only(true)
            .max_age(cookie::time::Duration::seconds(10))
            .inner(),
        )
        .unwrap();

      let cookie_deleted = cookie::Cookie::build(("will_be_deleted", "will_be_deleted"));
      webview.set_cookie(cookie_deleted.inner()).unwrap();

      println!("Setting Cookies:");
      for cookie in webview.cookies().unwrap() {
        println!("\t{cookie}");
      }

      println!("After Deleting:");
      webview.delete_cookie(cookie_deleted.inner()).unwrap();
      for cookie in webview.cookies().unwrap() {
        println!("\t{cookie}");
      }

      self.window = Some(window);
      self.webview = Some(webview);
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
          let webview = self.webview.as_ref().unwrap();
          let size = size.to_logical::<u32>(window.scale_factor());
          webview
            .set_bounds(Rect {
              position: LogicalPosition::new(0, 0).into(),
              size: LogicalSize::new(size.width, size.height).into(),
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

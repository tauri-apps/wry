// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Cookie management example for GTK4 / webkit6.
//!
//! Demonstrates `set_cookie`, `delete_cookie`, `cookies()`, and
//! `WebContext::set_cookie_accept_policy`.
//!
//! With `CookieAcceptPolicy::Never` active, cookies sent by HTTP Set-Cookie
//! headers (e.g. from httpbin's redirect URL) are silently blocked. Cookies
//! written programmatically via `WebView::set_cookie` bypass the policy and
//! still work — you should see `foo1` in the list but not `foo` from httpbin.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_cookies
//! ```

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
  use webkit6::CookieAcceptPolicy;
  use wry::{WebContext, WebViewBuilderExtUnix};

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Cookies (GTK4 / Wayland)"));
    window.set_default_size(800, 600);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    // Block all cookies arriving via HTTP Set-Cookie headers.
    // Programmatic set_cookie() / delete_cookie() calls bypass this policy.
    let mut ctx = WebContext::default();
    ctx.set_cookie_accept_policy(CookieAcceptPolicy::Never);
    println!("[policy] CookieAcceptPolicy::Never");
    println!("         HTTP Set-Cookie headers are blocked — 'foo=bar' from httpbin will not appear");
    println!("         Programmatic set_cookie() calls bypass the policy and still work");

    let webview = wry::WebViewBuilder::new_with_web_context(&mut ctx)
      // httpbin would normally set foo=bar via Set-Cookie; blocked by Never policy.
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

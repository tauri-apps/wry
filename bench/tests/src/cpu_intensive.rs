// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::process::exit;
use winit::{
  application::ApplicationHandler,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
  window::WindowId,
};
#[cfg(not(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
)))]
use winit::window::Window;
use wry::WebViewBuilder;

#[derive(Default)]
struct App {
  webview: Option<wry::WebView>,
  #[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  )))]
  window: Option<Window>,
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  gtk_window: Option<gtk4::Window>,
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    use wry::http::{header::CONTENT_TYPE, Request, Response};

    let handler = |req: Request<String>| {
      if req.body() == "process-complete" {
        exit(0);
      }
    };

    let builder = WebViewBuilder::new()
      .with_custom_protocol("wrybench".into(), move |_id, request| {
        let path = request.uri().to_string();
        let requested_asset_path = path.strip_prefix("wrybench://localhost").unwrap();
        let (data, mimetype): (_, String) = match requested_asset_path {
          "/index.css" => (
            include_bytes!("static/index.css").as_slice().into(),
            "text/css".into(),
          ),
          "/site.js" => (
            include_bytes!("static/site.js").as_slice().into(),
            "text/javascript".into(),
          ),
          "/worker.js" => (
            include_bytes!("static/worker.js").as_slice().into(),
            "text/javascript".into(),
          ),
          _ => (
            include_bytes!("static/index.html").as_slice().into(),
            "text/html".into(),
          ),
        };

        Response::builder()
          .header(CONTENT_TYPE, mimetype)
          .body(data)
          .unwrap()
      })
      .with_url("wrybench://localhost")
      .with_ipc_handler(handler);

    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    ))]
    {
      let _ = event_loop;
      use gtk4::prelude::*;
      use wry::WebViewBuilderExtUnix;
      let gtk_window = gtk4::Window::new();
      gtk_window.present();
      self.webview = Some(builder.build_gtk(&gtk_window).unwrap());
      self.gtk_window = Some(gtk_window);
      return;
    }

    #[cfg(not(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    )))]
    {
      let window = event_loop.create_window(Window::default_attributes()).unwrap();
      self.webview = Some(builder.build(&window).unwrap());
      self.window = Some(window);
    }
  }

  fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
    if let WindowEvent::CloseRequested = event {
      event_loop.exit();
    }
  }

  fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    ))]
    {
      event_loop.set_control_flow(ControlFlow::Poll);
      while gtk4::glib::MainContext::default().iteration(false) {}
    }
  }
}

fn main() {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  gtk4::init().unwrap();

  let event_loop = EventLoop::new().unwrap();
  let mut app = App::default();
  event_loop.run_app(&mut app).unwrap();
}

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  imp::main()
}

#[cfg(not(feature = "protocol"))]
mod imp {
  pub fn main() -> wry::Result<()> {
    unimplemented!()
  }
}

#[cfg(feature = "protocol")]
mod imp {
  use std::path::PathBuf;

  use dpi::{LogicalPosition, LogicalSize};
  use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
  };
  use wry::{
    http::{header::CONTENT_TYPE, Request, Response},
    Rect, WebViewBuilder,
  };

  #[derive(Default)]
  struct App {
    window: Option<Window>,
    webview: Option<wry::WebView>,
  }

  impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
      let window = event_loop
        .create_window(Window::default_attributes())
        .unwrap();

      let webview = WebViewBuilder::new()
        .with_custom_protocol(
          "wry".into(),
          move |_webview_id, request| match get_wry_response(request) {
            Ok(r) => r.map(Into::into),
            Err(e) => http::Response::builder()
              .header(CONTENT_TYPE, "text/plain")
              .status(500)
              .body(e.to_string().as_bytes().to_vec())
              .unwrap()
              .map(Into::into),
          },
        )
        // tell the webview to load the custom protocol
        .with_url("wry://localhost")
        .build_as_child(&window)
        .unwrap();

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

  pub fn main() -> wry::Result<()> {
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

  fn get_wry_response(
    request: Request<Vec<u8>>,
  ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let path = request.uri().path();
    // Read the file content from file path
    let root = PathBuf::from("examples/custom_protocol");
    let path = if path == "/" {
      "index.html"
    } else {
      //  removing leading slash
      &path[1..]
    };
    let content = std::fs::read(std::fs::canonicalize(root.join(path))?)?;

    // Return asset contents and mime types based on file extentions
    // If you don't want to do this manually, there are some crates for you.
    // Such as `infer` and `mime_guess`.
    let mimetype = if path.ends_with(".html") || path == "/" {
      "text/html"
    } else if path.ends_with(".js") {
      "text/javascript"
    } else if path.ends_with(".png") {
      "image/png"
    } else if path.ends_with(".wasm") {
      "application/wasm"
    } else {
      unimplemented!();
    };

    Response::builder()
      .header(CONTENT_TYPE, mimetype)
      .body(content)
      .map_err(Into::into)
  }
}

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! GTK4-native equivalent of the `async_custom_protocol` example.
//!
//! Demonstrates `with_asynchronous_custom_protocol` where the response can be
//! sent from any thread via the `RequestAsyncResponder`.
//!
//! Run with:
//!
//! ```bash
//! cargo run --features protocol --example gtk_async_custom_protocol
//! ```
//!
//! See LINUX.md for troubleshooting tips.

fn main() -> wry::Result<()> {
  imp::main()
}

#[cfg(not(feature = "protocol"))]
mod imp {
  pub fn main() -> wry::Result<()> {
    unimplemented!("rerun with --features protocol")
  }
}

#[cfg(feature = "protocol")]
mod imp {
  use std::path::PathBuf;

  use wry::http::{header::CONTENT_TYPE, Request, Response};

  pub fn main() -> wry::Result<()> {
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
    {
      eprintln!("gtk_async_custom_protocol is a Linux/BSD-only example.");
      Ok(())
    }
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
      window.set_title(Some("Async Custom Protocol (GTK4 / Wayland)"));
      window.set_default_size(800, 600);

      let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
      window.set_child(Some(&vbox));
      window.present();

      let webview = wry::WebViewBuilder::new()
        .with_asynchronous_custom_protocol(
          "wry".into(),
          move |_webview_id, request, responder| {
            match get_wry_response(request) {
              Ok(http_response) => responder.respond(http_response),
              Err(e) => responder.respond(
                http::Response::builder()
                  .header(CONTENT_TYPE, "text/plain")
                  .status(500)
                  .body(e.to_string().as_bytes().to_vec())
                  .unwrap(),
              ),
            }
          },
        )
        .with_url("wry://localhost")
        .build_gtk(&vbox)
        .unwrap();

      let webview = RefCell::new(Some(webview));
      window.connect_close_request(move |_| {
        webview.borrow_mut().take();
        gtk4::glib::Propagation::Proceed
      });
    });

    app.run();
    Ok(())
  }

  fn get_wry_response(
    request: Request<Vec<u8>>,
  ) -> Result<http::Response<Vec<u8>>, Box<dyn std::error::Error>> {
    let path = request.uri().path();
    let root = PathBuf::from("examples/custom_protocol");
    let path = if path == "/" { "index.html" } else { &path[1..] };
    let content = std::fs::read(std::fs::canonicalize(root.join(path))?)?;

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

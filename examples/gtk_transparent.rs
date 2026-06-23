// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Transparent webview example using the GTK4 backend.
//!
//! There are three layers of background colour when using a transparent webview:
//! 1. The compositor window surface — made transparent via CSS.
//! 2. The webview itself — enabled with `with_transparent(true)`.
//! 3. The HTML page — uses a semi-transparent `background-color`.
//!
//! Requires a compositing window manager (standard on Wayland; optional on X11).
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_transparent
//! ```
//!
//! See LINUX.md for troubleshooting tips.

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
  {
    eprintln!("gtk_transparent is a Linux/BSD-only example.");
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
    window.set_title(Some("Transparent (GTK4 / Wayland)"));
    window.set_default_size(800, 600);
    window.set_decorated(false);

    // Layer 1: make the GTK window surface transparent via CSS so the
    // compositor can blend it against the desktop background.
    let provider = gtk4::CssProvider::new();
    provider.load_from_data("window, .background { background-color: transparent; }");
    if let Some(display) = gtk4::gdk::Display::default() {
      gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
      );
    }

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    // Layer 2: transparent webview.
    // Layer 3: semi-transparent HTML body.
    let webview = wry::WebViewBuilder::new()
      .with_transparent(true)
      .with_html(
        r#"<html>
          <body style="background-color:rgba(87,87,87,0.5);"></body>
          <script>
            window.onload = function() {
              document.body.innerText = `hello, ${navigator.userAgent}`;
            };
          </script>
        </html>"#,
      )
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

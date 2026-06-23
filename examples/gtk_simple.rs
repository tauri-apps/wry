// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Minimal webview example using the GTK4 backend.
//!
//! Works on both X11 and Wayland. Run with:
//!
//! ```bash
//! cargo run --example gtk_simple
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
    eprintln!("gtk_simple is a Linux-only example.");
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
  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    use std::cell::RefCell;

    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Simple (GTK4 / Wayland)"));
    window.set_default_size(800, 600);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    let webview = wry::WebViewBuilder::new()
      .with_url("https://tauri.app")
      .with_devtools(true)
      .build_gtk(&vbox)
      .unwrap();

    // Anchor the WebView to the window's lifetime via close-request.
    // Without this, webview drops when the activate closure returns,
    // tearing down the WebKit process and causing app.run() to exit immediately.
    let webview = RefCell::new(Some(webview));
    window.connect_close_request(move |_| {
      webview.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

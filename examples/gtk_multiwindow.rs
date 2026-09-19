// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Multiple independent GTK4 windows, each hosting a webview.
//!
//! IPC buttons let pages open new windows, close the current one, or rename it.
//! The application exits when the last window is closed.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_multiwindow
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
    eprintln!("gtk_multiwindow is a Linux/BSD-only example.");
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

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    open_window(app.clone());
  });

  app.run();
  Ok(())
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
fn open_window(app: gtk4::Application) {
  use std::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering},
  };

  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  static COUNTER: AtomicUsize = AtomicUsize::new(1);
  let n = COUNTER.fetch_add(1, Ordering::Relaxed);

  let window = gtk4::ApplicationWindow::new(&app);
  window.set_title(Some(&format!("Window {n} (GTK4 / Wayland)")));
  window.set_default_size(800, 600);

  let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
  window.set_child(Some(&vbox));
  window.present();

  let app_clone = app.clone();
  let window_clone = window.clone();

  let webview = wry::WebViewBuilder::new()
    .with_html(
      r#"
        <button onclick="window.ipc.postMessage('new-window')">Open a new window</button>
        <button onclick="window.ipc.postMessage('close')">Close current window</button>
        <input oninput="window.ipc.postMessage(`change-title:${this.value}`)" />
      "#,
    )
    .with_ipc_handler(move |req: wry::http::Request<String>| {
      let body = req.body();
      match body.as_str() {
        "new-window" => open_window(app_clone.clone()),
        "close" => window_clone.close(),
        _ if body.starts_with("change-title:") => {
          let title = body.trim_start_matches("change-title:");
          window_clone.set_title(Some(title));
        }
        _ => {}
      }
    })
    .build_gtk(&vbox)
    .unwrap();

  let webview = RefCell::new(Some(webview));
  window.connect_close_request(move |_| {
    webview.borrow_mut().take();
    gtk4::glib::Propagation::Proceed
  });
}

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! GTK4-native equivalent of the `custom_titlebar` example.
//!
//! Builds a custom header bar using GTK4 widgets: a `WindowHandle` provides the
//! native drag region and a row of `Button` widgets handle minimize, maximize,
//! and close.  The webview fills the remaining space below.
//!
//! On GTK4/Wayland the compositor manages window dragging — `WindowHandle`
//! plugs into the compositor protocol directly without requiring IPC.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_custom_titlebar
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
    eprintln!("gtk_custom_titlebar is a Linux/BSD-only example.");
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
    window.set_title(Some("Custom Titlebar (GTK4 / Wayland)"));
    window.set_default_size(800, 600);
    window.set_decorated(false);

    // Root vertical box: [titlebar_row] + [webview_box]
    let outer = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&outer));

    // --- Titlebar row ---
    let titlebar_row = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
    titlebar_row.add_css_class("titlebar-row");
    outer.append(&titlebar_row);

    // WindowHandle marks its subtree as a compositor-level drag region.
    let drag_handle = gtk4::WindowHandle::new();
    drag_handle.set_hexpand(true);
    let title_label = gtk4::Label::new(Some("Custom Titlebar"));
    title_label.set_halign(gtk4::Align::Start);
    title_label.set_margin_start(8);
    drag_handle.set_child(Some(&title_label));
    titlebar_row.append(&drag_handle);

    // Window control buttons
    let btn_min = gtk4::Button::with_label("─");
    let btn_max = gtk4::Button::with_label("□");
    let btn_close = gtk4::Button::with_label("✕");
    for btn in [&btn_min, &btn_max, &btn_close] {
      btn.set_width_request(30);
      btn.set_height_request(30);
    }
    titlebar_row.append(&btn_min);
    titlebar_row.append(&btn_max);
    titlebar_row.append(&btn_close);

    // Apply minimal CSS so the titlebar is visually distinct.
    let css = gtk4::CssProvider::new();
    css.load_from_data(
      ".titlebar-row {
        background-color: #1f1f1f;
        color: white;
        min-height: 30px;
      }
      .titlebar-row button {
        background: none;
        border: none;
        color: white;
        border-radius: 0;
        min-height: 30px;
        min-width: 30px;
        padding: 0;
      }
      .titlebar-row button:hover { background-color: #3b3b3b; }
      label { color: white; }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
      gtk4::style_context_add_provider_for_display(
        &display,
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
      );
    }

    // --- Webview area ---
    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    vbox.set_vexpand(true);
    outer.append(&vbox);

    window.present();

    let webview = wry::WebViewBuilder::new()
      .with_html(
        r#"<html>
          <body style="display:grid;place-items:center;height:100vh;font-family:sans-serif;">
            <h4>WRYYYYYYYYYYYYYYYYYYYYYY!</h4>
          </body>
        </html>"#,
      )
      .build_gtk(&vbox)
      .unwrap();

    // Wire up button callbacks — GTK4 callbacks run on the main thread directly.
    let win_min = window.clone();
    btn_min.connect_clicked(move |_| win_min.minimize());

    let win_max = window.clone();
    btn_max.connect_clicked(move |_| {
      if win_max.is_maximized() {
        win_max.unmaximize();
      } else {
        win_max.maximize();
      }
    });

    let win_close = window.clone();
    btn_close.connect_clicked(move |_| win_close.close());

    let webview = RefCell::new(Some(webview));
    window.connect_close_request(move |_| {
      webview.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

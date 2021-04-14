// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(not(target_os = "linux"))]
fn main() {}

#[cfg(target_os = "linux")]
fn main() -> wry::Result<()> {
  use gio::{prelude::*, Cancellable};
  use gtk::prelude::*;
  use wry::webview::WebViewBuilder;

  gtk::init()?;
  let app = gtk::Application::new(Some("org.tauri.demo"), gio::ApplicationFlags::FLAGS_NONE)?;
  let cancellable: Option<&Cancellable> = None;
  app.register(cancellable)?;

  let window = gtk::ApplicationWindow::new(&app);
  window.set_default_size(320, 200);
  window.set_title("Basic example");
  window.show_all();

  let mut webview = WebViewBuilder::new(window)
    .unwrap()
    .initialize_script("menacing = 'ゴ';")
    .load_url("https://tauri.studio")?
    .build()?;
  webview.dispatch_script("console.log('Hello World');")?;

  loop {
    webview.evaluate_script()?;

    gtk::main_iteration();
  }
}

// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{event_loop::EventLoop, window::WindowBuilder},
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;

  let _webview = WebViewBuilder::new(window)?
    .with_url("https://tauri.studio")?
    .build()?;

  // webview do not need to be initialized to
  // access the version
  let version = _webview.version()?;
  println!("version {}", version);

  Ok(())
}

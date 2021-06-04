// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{fs, path::PathBuf};

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  // Use a sample directory at the root of the project
  let mut test_path = PathBuf::from("./target/webview_data");
  // The directory need to exist or the Webview will panic
  fs::create_dir_all(&test_path)?;
  // We need an absoulte path for the webview
  test_path = fs::canonicalize(&test_path)?;
  // The directory need to exist or the Webview will panic
  println!("Webview storage path: {:#?}", &test_path);

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();
  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url("https://tauri.studio")?
    .with_data_directory(test_path)
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

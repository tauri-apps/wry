// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const PAGE1_HTML: &[u8] = include_bytes!("custom_protocol_page1.html");

fn main() -> wry::Result<()> {
  use std::{
    fs::{canonicalize, read},
    path::PathBuf,
  };

  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    http::{header::CONTENT_TYPE, Response},
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Custom Protocol")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry".into(), move |request| {
      let path = request.uri().path();
      // Read the file content from file path
      let content = if path == "/" {
        PAGE1_HTML.into()
      } else {
        // `1..` for removing leading slash
        read(canonicalize(PathBuf::from("examples").join(&path[1..]))?)?.into()
      };

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
    })
    // tell the webview to load the custom protocol
    .with_url("wry://localhost")?
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

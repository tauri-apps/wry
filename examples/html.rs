// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use std::fs::{canonicalize, read};

  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    // We still register custom protocol here to show that how the page with http:// origin can
    // load them.
    .with_custom_protocol("wry".into(), move |requested_asset_path| {
      // Remove url scheme
      let path = requested_asset_path.replace("wry://", "");
      // Read the file content from file path
      let content = read(canonicalize(&path)?)?;

      // Return asset contents and mime types based on file extentions
      // If you don't want to do this manually, there are some crates for you.
      // Such as `infer` and `mime_guess`.
      if path.ends_with(".html") {
        Ok((content, String::from("text/html")))
      } else if path.ends_with(".js") {
        Ok((content, String::from("text/javascript")))
      } else if path.ends_with(".png") {
        Ok((content, String::from("image/png")))
      } else {
        unimplemented!();
      }
    })
    // tell the webview to load the html string
    .with_html(
      r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta http-equiv="X-UA-Compatible" content="IE=edge" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
</head>
<body>
  <h1>Welcome to WRY!</h1>
  <a href="wry://examples/hello.html">Link</a>
  <script type="text/javascript" src="wry://examples/hello.js"></script>
  <img src="wry://examples/icon.png" />
</body>
</html>"#,
    )?
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

// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::process::exit;

#[derive(Debug, Serialize, Deserialize)]
struct MessageParameters {
  message: String,
}

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::{Window, WindowBuilder},
    },
    http::ResponseBuilder,
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let handler = |_window: &Window, req: String| {
    if &req == "dom-loaded" {
      exit(0);
    }
  };
  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_ipc_handler(handler)
    .with_custom_protocol("wry.bench".into(), move |_request| {
      let index_html = r#"
      <!DOCTYPE html>
      <html lang="en">
        <head>
          <meta charset="UTF-8" />
          <meta http-equiv="X-UA-Compatible" content="IE=edge" />
          <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        </head>
        <body>
          <h1>Welcome to WRY!</h1>
          <script>
            document.addEventListener('DOMContentLoaded', () => {
              ipc.postMessage('dom-loaded')
            })
          </script>
        </body>
      </html>"#;

      ResponseBuilder::new()
        .mimetype("text/html")
        .body(index_html.into())
    })
    .with_url("wry.bench://")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => {
        let _ = webview.resize();
      }
    }
  });
}

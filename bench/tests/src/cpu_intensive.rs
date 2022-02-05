// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::process::exit;

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
    if &req == "process-complete" {
      exit(0);
    }
  };
  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry.bench".into(), move |request| {
      let requested_asset_path = request.uri().replace("wry.bench://", "");
      let (data, mimetype) = match requested_asset_path.as_str() {
        "/index.css" => (
          include_bytes!("static/index.css").to_vec(),
          "text/css".into(),
        ),
        "/site.js" => (
          include_bytes!("static/site.js").to_vec(),
          "text/javascript".into(),
        ),
        "/worker.js" => (
          include_bytes!("static/worker.js").to_vec(),
          "text/javascript".into(),
        ),
        _ => (
          include_bytes!("static/index.html").to_vec(),
          "text/html".into(),
        ),
      };

      ResponseBuilder::new().mimetype(mimetype).body(data)
    })
    .with_url("wry.bench://")?
    .with_ipc_handler(handler)
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


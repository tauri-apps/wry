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
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let handler = |_window: &Window, req: RpcRequest| {
    if &req.method == "process-complete" {
      exit(0);
    }
    None
  };
  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry.bench".into(), move |_, requested_asset_path| {
      let requested_asset_path = requested_asset_path.replace("wry.bench://", "");
      match requested_asset_path.as_str() {
        "/index.css" => Ok(include_bytes!("static/index.css").to_vec()),
        "/site.js" => Ok(include_bytes!("static/site.js").to_vec()),
        "/worker.js" => Ok(include_bytes!("static/worker.js").to_vec()),
        _ => Ok(include_bytes!("static/index.html").to_vec()),
      }
    })
    .with_url("wry.bench://")?
    .with_rpc_handler(handler)
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

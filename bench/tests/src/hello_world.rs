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
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let url = r#"data:text/html,
    <script>
    document.addEventListener('DOMContentLoaded', () => {
      rpc.call('dom-loaded')
    })
    </script>
  "#;

  let handler = |_window: &Window, req: RpcRequest| {
    if &req.method == "dom-loaded" {
      exit(0);
    }
    None
  };
  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url(url)?
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

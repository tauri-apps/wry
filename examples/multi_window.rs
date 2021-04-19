// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};

#[derive(Debug, Serialize, Deserialize)]
struct MessageParameters {
  message: String,
}

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      dpi::PhysicalSize,
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let (window_tx, window_rx) = std::sync::mpsc::channel::<String>();
  let handler = move |req: RpcRequest| {
    if &req.method == "openWindow" {
      if let Some(params) = req.params {
        if let Value::String(url) = &params[0] {
          let _ = window_tx.send(url.to_string());
        }
      }
    }
    None
  };

  let mut webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url("https://tauri.studio")?
    .with_initialization_script(
      r#"async function openWindow() {
                await window.rpc.notify("openWindow", "https://i.imgur.com/x6tXcr9.gif");
            }"#,
    )
    .with_rpc_handler(handler)
    .build()?;

  let instant = Instant::now();
  let eight_secs = Duration::from_secs(8);
  let mut new_webview = None;
  event_loop.run(move |event, event_loop, control_flow| {
    *control_flow = ControlFlow::Poll;

    if let Ok(url) = window_rx.try_recv() {
      let new_window = WindowBuilder::new()
        .with_title("RODA RORA DA")
        .with_inner_size(PhysicalSize::new(426, 197))
        .build(&event_loop)
        .unwrap();
      new_webview = Some(
        WebViewBuilder::new(new_window)
          .unwrap()
          .with_url(&url)
          .unwrap()
          .build()
          .unwrap(),
      );
    } else if let None = new_webview {
      if instant.elapsed() >= eight_secs {
        webview.dispatch_script("openWindow()").unwrap();
      }
    }

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => webview.evaluate_script().unwrap(),
    }
  });
}

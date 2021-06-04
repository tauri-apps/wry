// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

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
      window::{Window, WindowBuilder},
    },
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
  let window1 = WindowBuilder::new().build(&event_loop).unwrap();

  let (window_tx, window_rx) = std::sync::mpsc::channel::<String>();
  let handler = move |_window: &Window, req: RpcRequest| {
    if &req.method == "openWindow" {
      if let Some(params) = req.params {
        if let Value::String(url) = &params[0] {
          let _ = window_tx.send(url.to_string());
        }
      }
    }
    None
  };

  let id = window1.id();
  let webview1 = WebViewBuilder::new(window1)
    .unwrap()
    .with_url("https://tauri.studio")?
    .with_initialization_script(
      r#"async function openWindow() {
                await window.rpc.notify("openWindow", "https://i.imgur.com/x6tXcr9.gif");
            }"#,
    )
    .with_rpc_handler(handler)
    .build()?;
  let mut webviews = HashMap::new();
  webviews.insert(id, webview1);

  let instant = Instant::now();
  let eight_secs = Duration::from_secs(8);
  let mut trigger = true;
  event_loop.run(move |event, event_loop, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Ok(url) = window_rx.try_recv() {
      let window2 = WindowBuilder::new()
        .with_title("RODA RORA DA")
        .with_inner_size(PhysicalSize::new(426, 197))
        .build(&event_loop)
        .unwrap();
      let id = window2.id();
      let webview2 = WebViewBuilder::new(window2)
        .unwrap()
        .with_url(&url)
        .unwrap()
        .build()
        .unwrap();
      webviews.insert(id, webview2);
    } else if trigger && instant.elapsed() >= eight_secs {
      webviews
        .get_mut(&id)
        .unwrap()
        .dispatch_script("openWindow()")
        .unwrap();
      trigger = false;
    }

    for webview in webviews.values() {
      webview.evaluate_script().unwrap();
    }

    if let Event::WindowEvent {
      window_id,
      event: WindowEvent::CloseRequested,
    } = event
    {
      webviews.remove(&window_id);
      if webviews.is_empty() {
        *control_flow = ControlFlow::Exit;
      }
    }
  });
}

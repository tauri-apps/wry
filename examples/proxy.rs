// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{ProxyConfig, ProxyEndpoint, WebViewBuilder};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Proxy Test")
    .build(&event_loop)
    .unwrap();

  let http_proxy = ProxyConfig::Http(ProxyEndpoint {
    host: "localhost".to_string(),
    port: "3128".to_string(),
  });

  let _webview = WebViewBuilder::new(&window)
    .with_proxy_config(http_proxy)
    .with_url("https://www.myip.com/")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit
    }
  });
}

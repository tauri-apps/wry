// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::webview::proxy::{ProxyConfig, ProxyConnection, ProxyEndpoint, ProxyType};

fn main() -> wry::Result<()> {
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
    .with_title("Proxy Test")
    .build(&event_loop)?;

  let http_proxy = ProxyConnection::Http(ProxyEndpoint {
    host: "xx.xx.xx.xx".to_string(),
    port: "xxxx".to_string(),
  });

  let _webview = WebViewBuilder::new(window)?
    .with_proxy_config(ProxyConfig {
      proxy_type: ProxyType::Http,
      proxy_connection: http_proxy,
    })
    .with_url("https://www.myip.com/")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

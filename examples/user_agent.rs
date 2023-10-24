// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{webview_version, WebViewBuilder};

fn main() -> wry::Result<()> {
  let current_version = env!("CARGO_PKG_VERSION");
  let current_webview_version = webview_version().unwrap();
  let user_agent_string = format!(
    "wry/{} ({}; {})",
    current_version,
    std::env::consts::OS,
    current_webview_version
  );

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();
  let _webview = WebViewBuilder::new(&window)
    .with_user_agent(&user_agent_string)
    .with_url("https://www.whatismybrowser.com/detect/what-is-my-user-agent")?
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

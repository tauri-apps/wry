// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::{webview_version, WebViewBuilder},
  };

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
    .build(&event_loop)?;
  let _webview = WebViewBuilder::new(window)?
    .with_user_agent(&user_agent_string)
    .with_url("https://www.whatismybrowser.com/detect/what-is-my-user-agent")?
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

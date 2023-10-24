// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

enum UserEvent {
  NewWindow(String),
}

fn main() -> wry::Result<()> {
  let html = r#"
    <body>
      <div>
        <p> WRYYYYYYYYYYYYYYYYYYYYYY! </p>
        <a href="https://www.wikipedia.org" target="_blank">Visit Wikipedia</a>
        <a href="https://www.github.com" target="_blank">(Try to) visit GitHub</a>
      </div>
    </body>
  "#;

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();
  let _webview = WebViewBuilder::new(&window)
    .with_html(html)?
    .with_new_window_req_handler(move |uri: String| {
      let submitted = proxy.send_event(UserEvent::NewWindow(uri.clone())).is_ok();

      submitted && uri.contains("wikipedia")
    })
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::NewWindow(uri)) => {
        println!("New Window: {}", uri);
      }
      _ => (),
    }
  });
}

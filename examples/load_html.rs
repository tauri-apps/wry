// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
  enum UserEvent {
    LoadHtml,
  }

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();

  let window = WindowBuilder::new()
    .with_title("Load HTML")
    .build(&event_loop)
    .unwrap();

  let ipc_handler = move |req: String| {
    if req == "load-html" {
      let _ = proxy.send_event(UserEvent::LoadHtml);
    }
  };

  let webview = WebViewBuilder::new(&window)
    .with_html(
      r#"
      <button onclick="window.ipc.postMessage('load-html')">Load HTML</button>
    "#,
    )?
    .with_ipc_handler(ipc_handler)
    .build()?;

  const HTML: &str = r#"
  <html>
  <body>
      <h1> HTML LOADED </h1>
  </body>
  <script>
    alert("HTML Loaded");
  </script>
  </html>
"#;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::UserEvent(UserEvent::LoadHtml) => {
        webview.load_html(HTML);
      }
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

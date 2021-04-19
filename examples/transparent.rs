// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_decorations(false)
    .with_transparent(true)
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)?
    .transparent(true)
    .with_url(
      r#"data:text/html,
            <!doctype html>
            <html>
              <body style="background-color:rgba(87,87,87,0.);">hello</body>
              <script>
                window.onload = function() {
                  document.body.innerText = `hello, ${navigator.userAgent}`;
                };
              </script>
            </html>"#,
    )?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Poll;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

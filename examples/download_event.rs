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
    webview::WebViewBuilder,
  };
  
  let html = r#"
    <body>
      <div>
        <p> WRYYYYYYYYYYYYYYYYYYYYYY! </p>
        <a download="hello.txt" href='#allow' id="link">Allowed Download</a>
        <a download="hello.txt" href='#deny' id="link">Denied Download</a>
        <script>
        const blob = new Blob(["Hello, world!"], {type: 'text/plain'});
        link.href = URL.createObjectURL(blob);
        </script>
      </div>
    </body>
  "#;

  enum UserEvent {
    Download(String),
  }

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_download_handler(move |uri: String| {
      let submitted = proxy.send_event(UserEvent::Download(uri.clone())).is_ok();

      submitted && uri.contains("allow")
    })
    .build()?;

  #[cfg(debug_assertions)]
  webview.devtool();

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::Download(uri)) => {
        println!("Download: {}", uri);
      }
      _ => (),
    }
  });
}

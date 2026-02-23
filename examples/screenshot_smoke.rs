// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};
use std::time::Duration;

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::{PageLoadEvent, WebViewBuilder};

#[derive(Debug, Clone, Copy)]
enum UserEvent {
  Capture,
  Exit,
}

fn main() -> wry::Result<()> {
  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("wry screenshot smoke")
    .build(&event_loop)
    .unwrap();

  let already_requested = Arc::new(AtomicBool::new(false));
  let already_requested_ = already_requested.clone();
  let proxy_for_load = proxy.clone();

  let builder = WebViewBuilder::new()
    .with_html(
      r#"<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>WRY Screenshot Smoke</title>
    <style>
      html, body {
        margin: 0;
        width: 100%;
        height: 100%;
        font-family: sans-serif;
      }
      body {
        display: grid;
        place-items: center;
        background: #1f2937;
        color: white;
      }
      .card {
        padding: 24px 28px;
        border-radius: 16px;
        background: rgba(255,255,255,0.12);
        border: 1px solid rgba(255,255,255,0.18);
        box-shadow: 0 20px 50px rgba(0,0,0,0.35);
        backdrop-filter: blur(8px);
      }
      h1 { margin: 0 0 8px; font-size: 28px; }
      p { margin: 0; opacity: 0.9; }
    </style>
  </head>
  <body>
    <div class="card">
      <h1>Screenshot Smoke Test</h1>
      <p>If you can read this in screenshot.png, capture worked.</p>
    </div>
  </body>
</html>"#,
    )
    .with_on_page_load_handler(move |event, _url| {
      if matches!(event, PageLoadEvent::Finished)
        && !already_requested_.swap(true, Ordering::SeqCst)
      {
        let proxy = proxy_for_load.clone();
        std::thread::spawn(move || {
          std::thread::sleep(Duration::from_millis(1000));
          let _ = proxy.send_event(UserEvent::Capture);
        });
      }
    });

  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  let webview = builder.build(&window)?;
  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let webview = {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let vbox = window.default_vbox().unwrap();
    builder.build_gtk(vbox)?
  };

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::Capture) => {
        let proxy = proxy.clone();
        webview
          .screenshot(move |result| {
            match result {
              Ok(bytes) => {
                if let Err(err) = std::fs::write("screenshot.png", bytes) {
                  eprintln!("failed to write screenshot.png: {err}");
                } else {
                  println!("wrote screenshot.png");
                }
              }
              Err(err) => eprintln!("screenshot failed: {err}"),
            }
            let _ = proxy.send_event(UserEvent::Exit);
          })
          .expect("failed to request screenshot");
      }
      Event::UserEvent(UserEvent::Exit) => *control_flow = ControlFlow::Exit,
      _ => {}
    }
  });
}

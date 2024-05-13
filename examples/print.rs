// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::http::Request;
use wry::WebViewBuilder;

enum UserEvent {
  Print,
}

fn main() -> wry::Result<()> {
  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  let builder = WebViewBuilder::new(&window);

  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let builder = {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let vbox = window.default_vbox().unwrap();
    WebViewBuilder::new_gtk(vbox)
  };

  let ipc_handler = move |req: Request<String>| {
    let body = req.body();
    match body.as_str() {
      "print" => {
        let _ = proxy.send_event(UserEvent::Print);
      }
      _ => {}
    }
  };

  let webview = builder
    .with_html(
      r#"
          <button onclick="window.ipc.postMessage('print')">Print window</button>
        "#,
    )
    .with_ipc_handler(ipc_handler)
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::Print) => {
        #[cfg(target_os = "macos")]
        {
          use wry::{PrintOptions, WebViewExtMacOS};

          let print_options = PrintOptions {
            margins: wry::PrintMargin {
              top: 20.0,
              right: 0.0,
              bottom: 0.0,
              left: 20.0,
            },
          };

          webview.print_with_options(&print_options).unwrap();
        }

        #[cfg(not(target_os = "macos"))]
        webview.print().unwrap();
      }
      _ => {}
    }
  });
}

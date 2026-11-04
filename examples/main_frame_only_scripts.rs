// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Checks that initialization scripts registered for the main frame only do not run in subframes.
//!
//! The page embeds a sandboxed iframe. A main-frame-only script defines `window.__MAIN_FRAME_ONLY__`
//! and an all-frames script defines `window.__ALL_FRAMES__`. The iframe reports what it sees to the
//! parent via `postMessage`, the parent forwards both reports through IPC and the example exits with
//! status 0 when the main-frame-only global is missing in the iframe, 1 otherwise.

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

const HTML: &str = r#"<!doctype html>
<html>
  <body>
    <p>main frame</p>
    <script>
      const main = { mainOnly: typeof window.__MAIN_FRAME_ONLY__, all: typeof window.__ALL_FRAMES__ };
      window.addEventListener('message', (event) => {
        window.ipc.postMessage(JSON.stringify({ main, subframe: JSON.parse(event.data) }));
      });
    </script>
    <iframe
      sandbox="allow-scripts"
      srcdoc="<script>parent.postMessage(JSON.stringify({ mainOnly: typeof window.__MAIN_FRAME_ONLY__, all: typeof window.__ALL_FRAMES__ }), '*')</script>"
    ></iframe>
  </body>
</html>"#;

const EXPECTED: &str = r#"{"main":{"mainOnly":"string","all":"string"},"subframe":{"mainOnly":"undefined","all":"string"}}"#;

fn main() -> wry::Result<()> {
  let event_loop = EventLoopBuilder::<String>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("main frame only scripts")
    .build(&event_loop)
    .unwrap();

  let builder = WebViewBuilder::new()
    .with_initialization_script_for_main_only("window.__MAIN_FRAME_ONLY__ = 'secret';", true)
    .with_initialization_script_for_main_only("window.__ALL_FRAMES__ = 'shared';", false)
    .with_html(HTML)
    .with_ipc_handler(move |request| {
      let _ = proxy.send_event(request.into_body());
    });

  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  let _webview = builder.build(&window)?;
  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let _webview = {
    use tao::platform::unix::WindowExtUnix;
    use wry::WebViewBuilderExtUnix;
    let vbox = window.default_vbox().unwrap();
    builder.build_gtk(vbox)?
  };

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::UserEvent(report) => {
        println!("{report}");
        if report == EXPECTED {
          println!("ok: the subframe did not run the main-frame-only script");
          std::process::exit(0);
        } else {
          eprintln!("unexpected report, expected {EXPECTED}");
          std::process::exit(1);
        }
      }
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => {}
    }
  });
}

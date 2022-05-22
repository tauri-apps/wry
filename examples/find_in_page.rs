// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::webview::FindInPageOption;

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  #[derive(Debug)]
  enum UserEvent {
    FindInPage(String),
  }

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(
      r#"
    <input placeholder="Find text"/>
    <button>find</button>
    <p>Tauri is a toolkit that helps developers make applications for the major desktop platforms - using virtually any frontend framework in existence. The core is built with Rust, and the CLI leverages Node.js making Tauri a genuinely polyglot approach to creating and maintaining great apps. If you want to know more about the technical details, then please visit the Introduction. If you want to know more about this project's philosophy - then keep reading.</p>
    <script>
      document.querySelector("button").addEventListener("click", () => {
        const text = document.querySelector("input").value;
        window.ipc.postMessage(text);
      });
    </script>
"#,
    )?
    .with_ipc_handler(move |_, text: String| {
      proxy
        .send_event(UserEvent::FindInPage(text.clone()))
        .unwrap();
    });

  #[cfg(debug_assertions)]
  let webview = webview.with_devtools(true);

  let webview = webview.build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::FindInPage(text)) => {
        webview.find_in_page(
          text,
          FindInPageOption {
            case_sensitive: true,
            max_match_count: 100,
            ..FindInPageOption::default()
          },
          |found| println!("Is found: {}", found),
        );
      }
      _ => (),
    }
  });
}

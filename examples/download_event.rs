// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tempfile::{tempdir, TempDir};

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
        <a download="allow.zip" href='https://file-examples.com/wp-content/uploads/2017/02/zip_2MB.zip' id="link">Allowed Download</a>
        <a download="deny.zip" href='https://file-examples.com/wp-content/uploads/2017/02/zip_5MB.zip' id="link">Denied Download</a>
      </div>
    </body>
  "#;

  enum UserEvent {
    Download(String, TempDir),
    Rejected(String),
  }

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_download_handler(move |uri: String, result_path: &mut String| {
      if uri.ends_with("zip_2MB.zip") {
        if let Ok(tempdir) = tempdir() {
          if let Ok(path) = tempdir.path().canonicalize() {
            let path = String::from(path.join("example.zip").to_string_lossy());
            *result_path = path;
            let submitted = proxy.send_event(UserEvent::Download(uri.clone(), tempdir)).is_ok();

            return submitted;
          }
        }
      }

      let _ = proxy.send_event(UserEvent::Rejected(uri.clone()));

      false
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
      Event::UserEvent(UserEvent::Download(uri, temp_dir)) => {
        println!("Download: {}", uri);
        println!("Written to: {:?}", temp_dir.path().join("example.zip"));

        let len = std::fs::metadata(temp_dir.path().join("example.zip")).expect("Open file").len();

        println!("File size: {}", (len / 1024) / 1024)
      },
      Event::UserEvent(UserEvent::Rejected(uri)) => {
        println!("Rejected download from: {}", uri)
      }
      _ => (),
    }
  });
}

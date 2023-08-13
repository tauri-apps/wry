// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use std::{env::temp_dir, path::PathBuf};
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoopBuilder},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  const HTML: &str = r#"
    <body>
      <div>
        <p> WRYYYYYYYYYYYYYYYYYYYYYY! </p>
        <a download="allow.zip" href='https://github.com/tauri-apps/wry/archive/refs/tags/wry-v0.13.3.zip' id="link">Allowed Download</a>
        <a download="deny.zip" href='https://github.com/tauri-apps/wry/archive/refs/tags/wry-v0.13.2.zip' id="link">Denied Download</a>
      </div>
    </body>
  "#;

  enum UserEvent {
    DownloadStarted(String, String),
    DownloadComplete(Option<PathBuf>, bool),
    Rejected(String),
  }

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let _webview = WebViewBuilder::new(window)?
    .with_html(HTML)?
    .with_download_started_handler({
      let proxy = proxy.clone();
      move |uri: String, default_path: &mut PathBuf| {
        if uri.contains("wry-v0.13.3") {
          let path = temp_dir().join("example.zip").as_path().to_path_buf();

          *default_path = path.clone();

          let submitted = proxy
            .send_event(UserEvent::DownloadStarted(uri, path.display().to_string()))
            .is_ok();

          return submitted;
        }

        let _ = proxy.send_event(UserEvent::Rejected(uri));

        false
      }
    })
    .with_download_completed_handler({
      let proxy = proxy;
      move |_uri, path, success| {
        let _ = proxy.send_event(UserEvent::DownloadComplete(path, success));
      }
    })
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(UserEvent::DownloadStarted(uri, temp_dir)) => {
        println!("Download: {}", uri);
        println!("Will write to: {:?}", temp_dir);
      }
      Event::UserEvent(UserEvent::DownloadComplete(path, success)) => {
        let path = path.map(|_| temp_dir().join("example.zip"));
        println!("Succeeded: {}", success);
        if let Some(path) = path {
          println!("Path: {}", path.to_string_lossy());
          let metadata = path.metadata().unwrap();
          println!("Size of {}Mb", (metadata.len() / 1024) / 1024)
        } else {
          println!("No output path")
        }
      }
      Event::UserEvent(UserEvent::Rejected(uri)) => {
        println!("Rejected download from: {}", uri)
      }
      _ => (),
    }
  });
}

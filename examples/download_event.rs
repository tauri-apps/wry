// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use normpath::PathExt;
use tempfile::{tempdir, TempDir, tempdir_in};

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
        <a download="allow.zip" href='https://github.com/tauri-apps/wry/archive/refs/tags/wry-v0.13.3.zip' id="link">Allowed Download</a>
        <a download="deny.zip" href='https://github.com/tauri-apps/wry/archive/refs/tags/wry-v0.13.2.zip' id="link">Denied Download</a>
      </div>
    </body>
  "#;

  enum UserEvent {
    DownloadStarted(String, String),
    DownloadComplete(String, bool),
    Rejected(String),
  }

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_download_handler(
      {
        let proxy = proxy.clone();
        move |uri: String, result_path: &mut String| {
          if uri.contains("wry-v0.13.3") {
            if let Some(documents) = dirs::download_dir() {
              if let Ok(tempdir) = tempdir_in(documents) {
                if let Ok(path) = tempdir.path().normalize() {
                  dbg!(path.metadata().unwrap().permissions().readonly());
                  let path = path.join("example.zip").as_path().display().to_string();
                  *result_path = path;
                  let submitted = proxy.send_event(UserEvent::DownloadStarted(uri.clone(), result_path.clone())).is_ok();

                  return submitted;
                }
              }
            }
          }

          let _ = proxy.send_event(UserEvent::Rejected(uri.clone()));

          false
        }
      },
      {
        let proxy = proxy.clone();
        move || {
          let proxy = proxy.clone();
          Box::new(move |path, success| {
            let _ = proxy.send_event(UserEvent::DownloadComplete(path, success));
          })
        }
      }
    )
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
      Event::UserEvent(UserEvent::DownloadStarted(uri, temp_dir)) => {
        println!("Download: {}", uri);
        println!("Will write to: {:?}", temp_dir);
      },
      Event::UserEvent(UserEvent::DownloadComplete(path, success)) => {
        let metadata = PathBuf::from(&path).metadata();
        println!("Succeeded: {}", success);
        println!("Path: {}", path);
        if let Ok(metadata) = metadata {
          println!("Size of {}Mb", (metadata.len() / 1024) / 1024)
        } else {
          println!("Failed to retrieve file metadata - does it exist?")
        }
      },
      Event::UserEvent(UserEvent::Rejected(uri)) => {
        println!("Rejected download from: {}", uri)
      }
      _ => (),
    }
  });
}

// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use normpath::PathExt;
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
    DownloadStarted(String, TempDir),
    DownloadComplete(String),
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
          if uri.ends_with("zip_2MB.zip") {
            if let Ok(tempdir) = tempdir() {
              if let Ok(path) = tempdir.path().normalize() {
                let path = path.join("example.zip").as_path().display().to_string();
                *result_path = path;
                let submitted = proxy.send_event(UserEvent::DownloadStarted(uri.clone(), tempdir)).is_ok();

                return submitted;
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
            let _ = proxy.send_event(UserEvent::DownloadComplete(path));
          })
        }
      }
    )
    .build()?;

  #[cfg(debug_assertions)]
  webview.devtool();

  let mut temp_dir_holder = None;
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
        println!("Written to: {:?}", temp_dir.path().join("example.zip"));

        temp_dir_holder = Some(temp_dir);
      },
      Event::UserEvent(UserEvent::DownloadComplete(path)) => {
        let metadata = PathBuf::from(&path).metadata();
        if let Ok(metadata) = metadata {
          println!("File written to {}, size of {}Mb", path, (metadata.len() / 1024) / 1024)
        } else {
          println!("Failed to retrieve file metadata - does it exist?")
        }
        temp_dir_holder = None;
      },
      Event::UserEvent(UserEvent::Rejected(uri)) => {
        println!("Rejected download from: {}", uri)
      }
      _ => (),
    }
  });
}

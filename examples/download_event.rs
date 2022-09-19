// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use normpath::PathExt;
use tempfile::tempdir;

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
    DownloadComplete(Option<PathBuf>, bool),
    Rejected(String),
  }

  let temp_dir = Rc::new(RefCell::new(None));
  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let webview = WebViewBuilder::new(window)?
    .with_html(html)?
    .with_download_handler({
      let proxy = proxy.clone();
      let tempdir_cell = temp_dir.clone();
      move |uri: String, default_path: &mut PathBuf| {
        if uri.contains("wry-v0.13.3") {
          if let Ok(tempdir) = tempdir() {
            if let Ok(path) = tempdir.path().normalize() {
              tempdir_cell.borrow_mut().replace(tempdir);

              let path = path.join("example.zip").as_path().to_path_buf();

              *default_path = path.clone();

              let submitted = proxy
                .send_event(UserEvent::DownloadStarted(
                  uri.clone(),
                  path.display().to_string(),
                ))
                .is_ok();

              return submitted;
            }
          }
        }

        let _ = proxy.send_event(UserEvent::Rejected(uri.clone()));

        false
      }
    })
    .with_download_completed_handler({
      let proxy = proxy.clone();
      move |_uri, path, success| {
        let _ = proxy.send_event(UserEvent::DownloadComplete(path, success));
      }
    })
    .with_devtools(true)
    .build()?;

  #[cfg(debug_assertions)]
  webview.open_devtools();

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
      Event::UserEvent(UserEvent::DownloadComplete(mut path, success)) => {
        let _temp_dir_guard = if path.is_none() && success {
          let temp_dir = temp_dir.borrow_mut().take();
          path = Some(
            temp_dir
              .as_ref()
              .expect("Stored temp dir")
              .path()
              .join("example.zip"),
          );
          temp_dir
        } else {
          None
        };
        println!("Succeeded: {}", success);
        if let Some(path) = path {
          let metadata = path.metadata();
          println!("Path: {}", path.to_string_lossy());
          if let Ok(metadata) = metadata {
            println!("Size of {}Mb", (metadata.len() / 1024) / 1024)
          } else {
            println!("Failed to retrieve file metadata - does it exist?")
          }
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

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let builder = WebViewBuilder::new()
    .with_url("https://webrtc.github.io/samples/src/content/getusermedia/getdisplaymedia/")
    .with_new_window_req_handler(|url, features| {
      println!("new window req: {url} {features:?}");
      wry::NewWindowResponse::Allow
    });

  #[cfg(feature = "drag-drop")]
  let builder = builder.with_drag_drop_handler(|e| {
    match e {
      wry::DragDropEvent::Enter { paths, position } => {
        println!("DragEnter: {position:?} {paths:?} ")
      }
      wry::DragDropEvent::Over { position } => println!("DragOver: {position:?} "),
      wry::DragDropEvent::Drop { paths, position } => {
        println!("DragDrop: {position:?} {paths:?} ")
      }
      wry::DragDropEvent::Leave => println!("DragLeave"),
      _ => {}
    }

    true
  });

  #[cfg(any(
    target_os = "windows",
    target_os = "ios",
    target_os = "android"
  ))]
  let _webview = builder.build(&window)?;
  #[cfg(target_os = "macos")]
  let _webview = {
    use wry::WebViewBuilderExtMacos;
    builder.with_display_capture_decision_handler(|_capture_type| {
      wry::WKDisplayCapturePermissionDecision::ScreenPrompt
    }).build(&window)?
  };

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

  #[cfg(target_os = "macos")]
  {
    use wry::WebViewExtMacOS;
    _webview.set_display_capture_decision_handler(|_capture_type| {
      wry::WKDisplayCapturePermissionDecision::ScreenPrompt
    });
  }

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      *control_flow = ControlFlow::Exit;
    }
  });
}

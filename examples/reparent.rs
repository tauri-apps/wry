// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  dpi::LogicalPosition,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  platform::macos::WindowExtMacOS,
  window::WindowBuilder,
};
use wry::{WebViewBuilder, WebViewExtMacOS};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();
  let original_window_id = window.id();

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

  let webview = builder.with_url("https://tauri.app")?.build()?;

  let mut original_window = Some(window);
  let mut detached_window_ref: Option<tao::window::Window> = None;

  event_loop.run(move |event, target, control_flow| {
    *control_flow = ControlFlow::Wait;

    if let Event::WindowEvent {
      window_id,
      event: WindowEvent::CloseRequested,
      ..
    } = event
    {
      if window_id == original_window_id {
        original_window.take();

        let detached_window = WindowBuilder::new()
          .with_position(LogicalPosition::new(0, 0))
          .build(&target)
          .unwrap();

        webview.reparent(detached_window.ns_window() as cocoa::base::id);

        detached_window_ref.replace(detached_window);
      } else {
        *control_flow = ControlFlow::Exit
      }
    }
  });
}

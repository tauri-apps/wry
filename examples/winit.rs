// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "linux")]
fn main() {}

#[cfg(not(target_os = "linux"))]
fn main() -> wry::Result<()> {
  use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
  };
  use wry::webview::WebViewBuilder;

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();
  let webview = WebViewBuilder::new(window)
    .unwrap()
    .initialize_script("menacing = 'ゴ';")
    .load_url("wry://tauri.studio")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::WindowEvent {
        event: WindowEvent::Resized(_),
        ..
      } => {
        webview.resize().unwrap();
      }
      Event::MainEventsCleared => {
        webview.window().request_redraw();
      }
      Event::RedrawRequested(_) => {}
      _ => (),
    }
  });
}

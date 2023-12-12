// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  dpi::PhysicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{WebViewBuilder, WebViewBuilderExtServo, WebViewExtServo};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_inner_size(PhysicalSize::new(800, 800))
    .build(&event_loop)
    .unwrap();

  #[allow(unused_mut)]
  let mut builder = WebViewBuilder::new_servo(window, event_loop.create_proxy());
  let mut webview = builder.with_url("https://tauri.app")?.build()?;

  event_loop.run(move |event, evl, control_flow| {
    if webview.servo().is_shutdown() {
      if let Some(servo) = webview.servo().servo_client().take() {
        servo.deinit();
      }
      *control_flow = ControlFlow::Exit;
    } else {
      *control_flow = webview.servo().get_control_flow(&event);
      webview.servo().handle_tao_event(event);
      webview.servo().handle_servo_messages();
    }
  });

  Ok(())
}

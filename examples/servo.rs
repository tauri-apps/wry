// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  dpi::{PhysicalPosition, PhysicalSize},
  event::Event,
  event_loop::{ControlFlow, EventLoopBuilder},
  window::WindowBuilder,
};
use wry::{Rect, WebViewBuilder, WebViewBuilderExtServo, WebViewExtServo};

fn main() {
  let event_loop = EventLoopBuilder::<()>::with_user_event().build();
  let window = WindowBuilder::new()
    .with_inner_size(PhysicalSize::new(1000, 500))
    .build(&event_loop)
    .expect("failed to create demo window");
  let window_id = window.id();
  let proxy = event_loop.create_proxy();
  let mut webview = WebViewBuilder::new()
    .with_bounds(Rect {
      position: PhysicalPosition::new(20, 20).into(),
      size: PhysicalSize::new(960, 460).into(),
    })
    .build_servo_as_child(&window, move || {
      if let Err(error) = proxy.send_event(()) {
        eprintln!("failed to wake the Tao event loop: {error}");
      }
    })
    .expect("failed to create Servo webview");

  event_loop.run(move |event, _event_loop, control_flow| {
    let _keep_window_alive = &window;
    webview.servo().set_control_flow(control_flow);

    match event {
      Event::UserEvent(()) => webview.servo().handle_user_event(),
      Event::WindowEvent {
        window_id: event_window_id,
        event,
        ..
      } if event_window_id == window_id => webview.servo().handle_window_event(control_flow, event),
      Event::RedrawRequested(event_window_id) if event_window_id == window_id => {
        webview.servo().paint()
      }
      _ => {}
    }

    if webview.servo().is_shutdown() {
      *control_flow = ControlFlow::Exit;
    }
  });
}

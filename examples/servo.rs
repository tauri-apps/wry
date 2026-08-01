// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use winit::{
  application::ApplicationHandler,
  dpi::PhysicalSize,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
  window::{Window, WindowId},
};
use wry::{WebView, WebViewBuilder, WebViewBuilderExtServo, WebViewExtServo};

struct App {
  proxy: EventLoopProxy<()>,
  webview: Option<WebView>,
}

impl ApplicationHandler<()> for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.webview.is_some() {
      return;
    }

    let window = event_loop
      .create_window(Window::default_attributes().with_inner_size(PhysicalSize::new(1000, 500)))
      .expect("failed to create demo window");
    let webview = WebViewBuilder::new()
      .build_servo(window, self.proxy.clone())
      .expect("failed to create Servo webview");
    self.webview = Some(webview);
  }

  fn user_event(&mut self, _event_loop: &ActiveEventLoop, _event: ()) {
    if let Some(webview) = &mut self.webview {
      webview.servo().handle_user_event();
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _window_id: WindowId,
    event: WindowEvent,
  ) {
    if let Some(webview) = &mut self.webview {
      webview.servo().handle_window_event(event_loop, event);
    }
  }

  fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
    if let Some(webview) = &mut self.webview {
      if webview.servo().is_shutdown() {
        event_loop.exit();
      } else {
        webview.servo().set_control_flow(event_loop);
      }
    }
  }
}

fn main() {
  let event_loop = EventLoop::with_user_event()
    .build()
    .expect("failed to create event loop");
  let mut app = App {
    proxy: event_loop.create_proxy(),
    webview: None,
  };
  event_loop.run_app(&mut app).expect("Servo demo failed");
}

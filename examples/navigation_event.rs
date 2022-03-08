// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::{WebViewBuilder, WebContext},
  };

  enum UserEvent {
    Navigation(String)
  }

  let event_loop: EventLoop<UserEvent> = EventLoop::with_user_event();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;
  let mut web_context = WebContext::default();
  let webview = WebViewBuilder::new(window)?
    .with_url("http://neverssl.com")?
    .with_web_context(&mut web_context)
    .with_navigation_callback(move |uri: String, new_window: bool| {
      let submitted = proxy.send_event(UserEvent::Navigation(uri.clone())).is_ok();

      new_window || !submitted || !uri.contains("neverssl")
    })?
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
      Event::UserEvent(UserEvent::Navigation(uri)) => {
        println!("{}", uri);
      }
      _ => (),
    }
  });
}

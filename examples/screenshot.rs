// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
use std::{fs::File, io::Write};

use tao::{
  event::{Event, StartCause, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
  // Build our event loop
  let event_loop = EventLoop::new();
  let event_proxy = event_loop.create_proxy();
  // Build the window
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();
  // Build the webview
  let webview = WebViewBuilder::new_as_child(&window)
    .with_bounds(wry::Rect {
      position: dpi::Position::Logical(dpi::LogicalPosition { x: 50.0, y: 50.0 }),
      size: dpi::Size::Logical(dpi::LogicalSize {
        width: 600.0,
        height: 400.0,
      }),
    })
    .with_url("https://html5test.com")
    .with_on_page_load_handler(move |_, _| event_proxy.send_event(()).unwrap())
    .build()?;

  // launch WRY process
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::UserEvent(()) => {
        let on_screenshot = |image: wry::Result<Vec<u8>>| {
          let image = image.expect("No image?");
          let mut file = File::create("baaaaar.png").expect("Couldn't create the dang file");
          file
            .write(image.as_slice())
            .expect("Couldn't write the dang file");
        };
        webview.screenshot(on_screenshot).expect("Take screenshot")
      }
      _ => (),
    }
  });
}

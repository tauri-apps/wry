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
      webview::WebViewBuilder,
    };
    use ico::*;

    // use a window icon
    let file = std::fs::File::open("<specify_the_icon_here>").unwrap();
    let icon_dir = ico::IconDir::read(file).unwrap();
    let image = icon_dir.entries()[0].decode().unwrap();
    let rgba = image.rgba_data();
    let icon = WindowIcon::from_rgba(rgba.to_vec(), 256, 256)?;
  
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
      .with_title("Hello World")
      .with_window_icon(Some(icon))
      .build(&event_loop)?;
    let _webview = WebViewBuilder::new(window)?
      .with_url("https://tauri.studio")?
      .build()?;
  
    event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Poll;
  
      match event {
        Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => *control_flow = ControlFlow::Exit,
        _ => (),
      }
    });
  }
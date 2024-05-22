// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{
  dpi::{LogicalPosition, LogicalSize},
  Rect, WebViewBuilder,
};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  let fixed = {
    use gtk::prelude::*;
    use tao::platform::unix::WindowExtUnix;

    let fixed = gtk::Fixed::new();
    let vbox = window.default_vbox().unwrap();
    vbox.pack_start(&fixed, true, true, 0);
    fixed.show_all();
    fixed
  };

  let create_webview_builder = || {
    #[cfg(any(
      target_os = "windows",
      target_os = "macos",
      target_os = "ios",
      target_os = "android"
    ))]
    return WebViewBuilder::new_as_child(&window);

    #[cfg(not(any(
      target_os = "windows",
      target_os = "macos",
      target_os = "ios",
      target_os = "android"
    )))]
    {
      use wry::WebViewBuilderExtUnix;
      WebViewBuilder::new_gtk(&fixed)
    }
  };

  let size = window.inner_size().to_logical::<u32>(window.scale_factor());

  let webview = create_webview_builder()
    .with_bounds(Rect {
      position: LogicalPosition::new(0, 0).into(),
      size: LogicalSize::new(size.width / 2, size.height / 2).into(),
    })
    .with_url("https://tauri.app")
    .build()?;
  let webview2 = WebViewBuilder::new_as_child(&window)
    .with_bounds(Rect {
      position: LogicalPosition::new(size.width / 2, 0).into(),
      size: LogicalSize::new(size.width / 2, size.height / 2).into(),
    })
    .with_url("https://github.com/tauri-apps/wry")
    .build()?;
  let webview3 = WebViewBuilder::new_as_child(&window)
    .with_bounds(Rect {
      position: LogicalPosition::new(0, size.height / 2).into(),
      size: LogicalSize::new(size.width / 2, size.height / 2).into(),
    })
    .with_url("https://twitter.com/TauriApps")
    .build()?;
  let webview4 = WebViewBuilder::new_as_child(&window)
    .with_bounds(Rect {
      position: LogicalPosition::new(size.width / 2, size.height / 2).into(),
      size: LogicalSize::new(size.width / 2, size.height / 2).into(),
    })
    .with_url("https://google.com")
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event: WindowEvent::Resized(size),
        ..
      } => {
        let size = size.to_logical::<u32>(window.scale_factor());
        webview
          .set_bounds(Rect {
            position: LogicalPosition::new(0, 0).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webview2
          .set_bounds(Rect {
            position: LogicalPosition::new(size.width / 2, 0).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webview3
          .set_bounds(Rect {
            position: LogicalPosition::new(0, size.height / 2).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
        webview4
          .set_bounds(Rect {
            position: LogicalPosition::new(size.width / 2, size.height / 2).into(),
            size: LogicalSize::new(size.width / 2, size.height / 2).into(),
          })
          .unwrap();
      }
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => {}
    }
  });
}

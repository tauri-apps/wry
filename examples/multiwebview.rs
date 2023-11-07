// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use winit::{
  dpi::LogicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::WebViewBuilder;

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  {
    use gtk::prelude::DisplayExtManual;

    gtk::init()?;
    if gtk::gdk::Display::default().unwrap().backend().is_wayland() {
      panic!("This example doesn't support wayland!");
    }

    // we need to ignore this error here otherwise it will be catched by winit and will be
    // make the example crash
    winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
      let error = error as *mut x11_dl::xlib::XErrorEvent;
      (unsafe { (*error).error_code }) == 170
    }));
  }

  let event_loop = EventLoop::new().unwrap();
  let window = WindowBuilder::new()
    .with_inner_size(LogicalSize::new(800, 800))
    .build(&event_loop)
    .unwrap();

  let size = window.inner_size().to_logical::<u32>(window.scale_factor());

  let webview = WebViewBuilder::new_as_child(&window)
    .with_position((0, 0))
    .with_size((size.width / 2, size.height / 2))
    .with_url("https://tauri.app")?
    .build()?;
  let webview2 = WebViewBuilder::new_as_child(&window)
    .with_position(((size.width / 2) as i32, 0))
    .with_size((size.width / 2, size.height / 2))
    .with_url("https://github.com/tauri-apps/wry")?
    .build()?;
  let webview3 = WebViewBuilder::new_as_child(&window)
    .with_position((0, (size.height / 2) as i32))
    .with_size((size.width / 2, size.height / 2))
    .with_url("https://twitter.com/TauriApps")?
    .build()?;
  let webview4 = WebViewBuilder::new_as_child(&window)
    .with_position(((size.width / 2) as i32, (size.height / 2) as i32))
    .with_size((size.width / 2, size.height / 2))
    .with_url("https://google.com")?
    .build()?;

  event_loop
    .run(move |event, evl| {
      evl.set_control_flow(ControlFlow::Poll);

      #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
      ))]
      while gtk::events_pending() {
        gtk::main_iteration_do(false);
      }

      match event {
        Event::WindowEvent {
          event: WindowEvent::Resized(size),
          ..
        } => {
          let size = size.to_logical::<u32>(window.scale_factor());
          webview.set_size((size.width / 2, size.height / 2));
          webview.set_position((0, 0));
          webview2.set_position(((size.width / 2) as i32, 0));
          webview2.set_size((size.width / 2, size.height / 2));
          webview3.set_position((0, (size.height / 2) as i32));
          webview3.set_size((size.width / 2, size.height / 2));
          webview4.set_position(((size.width / 2) as i32, (size.height / 2) as i32));
          webview4.set_size((size.width / 2, size.height / 2));
        }
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => evl.exit(),
        _ => {}
      }
    })
    .unwrap();

  Ok(())
}

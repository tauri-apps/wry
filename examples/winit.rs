// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use winit::{
  dpi::PhysicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  window::WindowBuilder,
};
use wry::{WebViewBuilder, WebViewBuilderExtServo, WebViewExtServo};

fn main() -> wry::Result<()> {
  // #[cfg(any(
  //   target_os = "linux",
  //   target_os = "dragonfly",
  //   target_os = "freebsd",
  //   target_os = "netbsd",
  //   target_os = "openbsd",
  // ))]
  // {
  //   use gtk::prelude::DisplayExtManual;

  //   gtk::init().unwrap();
  //   if gtk::gdk::Display::default().unwrap().backend().is_wayland() {
  //     panic!("This example doesn't support wayland!");
  //   }

  //   // we need to ignore this error here otherwise it will be catched by winit and will be
  //   // make the example crash
  //   winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
  //     let error = error as *mut x11_dl::xlib::XErrorEvent;
  //     (unsafe { (*error).error_code }) == 170
  //   }));
  // }

  let event_loop = EventLoop::new().unwrap();
  let window = WindowBuilder::new()
    .with_inner_size(PhysicalSize::new(800, 800))
    .build(&event_loop)
    .unwrap();

  #[allow(unused_mut)]
  let mut builder = WebViewBuilder::new_servo(window, event_loop.create_proxy());
  let mut webview = builder.with_url("https://tauri.app")?.build()?;

  event_loop
    .run(move |event, evl| {
      if !evl.exiting() && webview.servo().is_shutdown() {
        if let Some(servo) = webview.servo().servo_client().take() {
          servo.deinit();
        }
        evl.exit();
      } else {
        webview.servo().set_control_flow(&event, evl);
        webview.servo().handle_winit_event(event);
        webview.servo().handle_servo_messages();
      }
    })
    .unwrap();

  Ok(())
}

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

/* window decoration */
use cocoa::appkit::{NSView, NSWindow};
use cocoa::appkit::{NSWindowStyleMask, NSWindowTitleVisibility};
use objc::{msg_send, runtime::Object, sel, sel_impl};
use raw_window_handle::{AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle};
use winit::dpi::LogicalPosition;
use winit::platform::macos::WindowBuilderExtMacOS;

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new().unwrap();
  let window = WindowBuilder::new()
    .with_inner_size(PhysicalSize::new(1000, 500))
    .build(&event_loop)
    .unwrap();

  unsafe {
    let rwh = window.raw_window_handle();
    if let RawWindowHandle::AppKit(AppKitWindowHandle { ns_window, .. }) = rwh {
      decorate_window(ns_window as *mut Object, LogicalPosition::new(8.0, 40.0));
    }
  }

  #[allow(unused_mut)]
  let mut builder = WebViewBuilder::new_servo(window, event_loop.create_proxy());
  let mut webview = builder.build()?;

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

pub unsafe fn decorate_window(window: *mut Object, position: LogicalPosition<f64>) {
  NSWindow::setTitlebarAppearsTransparent_(window, true);
  NSWindow::setTitleVisibility_(window, NSWindowTitleVisibility::NSWindowTitleHidden);
  NSWindow::setStyleMask_(
    window,
    NSWindowStyleMask::NSTitledWindowMask
      | NSWindowStyleMask::NSFullSizeContentViewWindowMask
      | NSWindowStyleMask::NSClosableWindowMask
      | NSWindowStyleMask::NSResizableWindowMask
      | NSWindowStyleMask::NSMiniaturizableWindowMask,
  );
}

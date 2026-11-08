// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tao::{
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop},
  platform::unix::WindowExtUnix,
  window::WindowBuilder,
};
#[cfg(target_os = "linux")]
use wry::{
  dpi::{LogicalPosition, LogicalSize},
  Rect, WebViewBuilder, WebViewBuilderExtUnix, WebViewExtUnix,
};

#[cfg(target_os = "linux")]
fn rect(x: i32, y: i32, width: i32, height: i32) -> Rect {
  Rect {
    position: LogicalPosition::new(x, y).into(),
    size: LogicalSize::new(width, height).into(),
  }
}

#[cfg(target_os = "linux")]
fn check(fixed: &gtk::Fixed, webview: &wry::WebView, expected: (i32, i32, i32, i32)) {
  let children = fixed.children();
  assert_eq!(children.len(), 1, "GtkFixed child count");

  let child = &children[0];
  let allocation = child.allocation();

  let gtk_bounds = (
    fixed.child_property::<i32>(child, "x"),
    fixed.child_property::<i32>(child, "y"),
    allocation.width(),
    allocation.height(),
  );

  let bounds = webview.bounds().expect("Wry bounds");
  let position = bounds.position.to_logical::<i32>(1.0);
  let size = bounds.size.to_logical::<i32>(1.0);

  let wry_bounds = (position.x, position.y, size.width, size.height);

  println!("BOUNDS expected={expected:?} gtk={gtk_bounds:?} wry={wry_bounds:?}");

  assert_eq!(gtk_bounds, expected, "GTK geometry");
  assert_eq!(wry_bounds, expected, "Wry geometry");
}

#[cfg(target_os = "linux")]
fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();

  let window_a = WindowBuilder::new()
    .with_title("Wry geometry source")
    .build(&event_loop)
    .unwrap();

  let window_b = WindowBuilder::new()
    .with_title("Wry geometry destination")
    .build(&event_loop)
    .unwrap();

  let source = window_a.default_vbox().unwrap().clone();
  let fixed = gtk::Fixed::new();
  fixed.set_hexpand(true);
  fixed.set_vexpand(true);

  window_b
    .default_vbox()
    .unwrap()
    .pack_start(&fixed, true, true, 0);
  fixed.show();

  let webview = WebViewBuilder::new()
    .with_html("<h1>Wry geometry regression</h1>")
    .build_gtk(&source)?;

  let windows = (window_a, window_b);
  let start = Instant::now();
  let mut phase = 0;

  event_loop.run(move |event, _, control_flow| {
    let _keep_windows_alive = &windows;
    *control_flow = ControlFlow::Poll;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => std::process::exit(1),

      Event::MainEventsCleared => {
        let elapsed = start.elapsed();

        if phase == 0 && elapsed >= Duration::from_millis(500) {
          webview.reparent(&fixed).unwrap();
          webview.set_bounds(rect(90, 70, 330, 240)).unwrap();
          phase = 1;
        }

        if phase == 1 && elapsed >= Duration::from_millis(1100) {
          check(&fixed, &webview, (90, 70, 330, 240));
          phase = 2;
        }

        if phase == 2 && elapsed >= Duration::from_millis(1600) {
          webview.reparent(&source).unwrap();
          assert!(fixed.children().is_empty());
          phase = 3;
        }

        if phase == 3 && elapsed >= Duration::from_millis(2100) {
          webview.reparent(&fixed).unwrap();
          webview.set_bounds(rect(120, 85, 280, 200)).unwrap();
          phase = 4;
        }

        if phase == 4 && elapsed >= Duration::from_millis(2700) {
          check(&fixed, &webview, (120, 85, 280, 200));
          println!("CONCISE_REGRESSION: PASS");
          *control_flow = ControlFlow::Exit;
          phase = 5;
        }
      }

      _ => {}
    }
  });
}

#[cfg(not(target_os = "linux"))]
fn main() {}

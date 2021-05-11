// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use std::collections::HashMap;
  #[cfg(target_os = "linux")]
  use std::path::Path;
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      menu::{MenuItem, MenuType},
      platform::system_tray::SystemTrayBuilder,
      window::Window,
    },
    webview::WebViewBuilder,
  };

  // Build our event loop
  let event_loop = EventLoop::new();
  let mut webviews = HashMap::new();

  // Create sample menu item
  let open_new_window = MenuItem::new("Open new window");
  let open_new_window_id = open_new_window.id();

  // Windows require Vec<u8> ICO file
  #[cfg(target_os = "windows")]
  let icon = include_bytes!("icon.ico").to_vec();
  // macOS require Vec<u8> PNG file
  #[cfg(target_os = "macos")]
  let icon = include_bytes!("icon.png").to_vec();
  // Linux require Pathbuf to PNG file
  #[cfg(target_os = "linux")]
  let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/icon.png");

  let _system_tray = SystemTrayBuilder::new(icon, vec![open_new_window])
    .build(&event_loop)
    .unwrap();

  // launch WRY process
  event_loop.run(move |event, event_loop, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        window_id,
      } => {
        println!("Window {:?} has received the signal to close", window_id);
        // Remove window from our hashmap
        webviews.remove(&window_id);
      }
      // Catch menu events
      Event::MenuEvent {
        menu_id,
        origin: MenuType::SystemTray,
      } => {
        if menu_id == open_new_window_id {
          let window = Window::new(&event_loop).unwrap();
          let id = window.id();
          let webview = WebViewBuilder::new(window)
            .unwrap()
            .with_url("https://tauri.studio")
            .unwrap()
            .build()
            .unwrap();
          webviews.insert(id, webview);
        }
        println!("Clicked on {:?}", menu_id);
      }
      _ => (),
    }
  });
}

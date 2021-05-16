// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use std::{fs::File, io::Write};
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      menu::{Menu, MenuItem, MenuType},
      window::WindowBuilder,
    },
    webview::{ScreenshotRegion, WebViewBuilder},
  };

  let custom_screenshot_visible = MenuItem::new("Visible");
  let custom_screenshot_fulldocument = MenuItem::new("Full document");
  let custom_screenshot_visible_id = custom_screenshot_visible.id();
  let custom_screenshot_fulldocument_id = custom_screenshot_fulldocument.id();

  // macOS require to have at least Copy, Paste, Select all etc..
  // to works fine. You should always add them.
  #[cfg(any(target_os = "linux", target_os = "macos"))]
  let menu = vec![
    Menu::new("File", vec![MenuItem::CloseWindow]),
    Menu::new(
      "Edit",
      vec![
        MenuItem::Undo,
        MenuItem::Redo,
        MenuItem::Separator,
        MenuItem::Cut,
        MenuItem::Copy,
        MenuItem::Paste,
        MenuItem::Separator,
        MenuItem::SelectAll,
      ],
    ),
    Menu::new(
      // on macOS first menu is always app name
      "Screenshot",
      vec![custom_screenshot_visible, custom_screenshot_fulldocument],
    ),
  ];

  // Attention, Windows only support custom menu for now.
  // If we add any `MenuItem::*` they'll not render
  // We need to use custom menu with `Menu::new()` and catch
  // the events in the EventLoop.
  #[cfg(target_os = "windows")]
  let menu = vec![Menu::new(
    "Screenshot",
    vec![custom_screenshot_visible, custom_screenshot_fulldocument],
  )];

  // Build our event loop
  let event_loop = EventLoop::new();
  // Build the window
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .with_menu(menu)
    .build(&event_loop)?;
  // Build the webview
  let webview = WebViewBuilder::new(window)?
    .with_url("https://html5test.com")?
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
      // Catch menu events
      Event::MenuEvent {
        menu_id,
        origin: MenuType::Menubar,
      } => {
        let on_screenshot = |image: wry::Result<Vec<u8>>| {
          let image = image.expect("No image?");
          let mut file = File::create("baaaaar.png").expect("Couldn't create the dang file");
          file
            .write(image.as_slice())
            .expect("Couldn't write the dang file");
        };
        if menu_id == custom_screenshot_visible_id {
          webview
            .screenshot(ScreenshotRegion::Visible, on_screenshot)
            .expect("Unable to screenshot");
        }
        if menu_id == custom_screenshot_fulldocument_id {
          webview
            .screenshot(ScreenshotRegion::FullDocument, on_screenshot)
            .expect("Unable to screenshot");
        }
        println!("Clicked on {:?}", menu_id);
      }
      _ => (),
    }
  });
}

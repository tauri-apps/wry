// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      menu::{Menu, MenuItem, MenuType},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  // `Primary` is a platform-agnostic accelerator modifier.
  // On Windows and Linux, `Primary` maps to the `Ctrl` key,
  // and on macOS it maps to the `command` key.
  let custom_print_menu = MenuItem::new("Print").with_accelerators("<Primary>p");
  let other_test_menu = MenuItem::new("Custom").with_accelerators("<Primary>M");
  let quit_menu = MenuItem::new("Quit").with_accelerators("<Primary>q");
  let custom_print_menu_id = custom_print_menu.id();
  let quit_menu_id = quit_menu.id();

  // macOS require to have at least Copy, Paste, Select all etc..
  // to works fine. You should always add them.
  #[cfg(any(target_os = "linux", target_os = "macos"))]
  let menu = vec![
    Menu::new(
      // on macOS first menu is always app name
      "my custom app",
      vec![
        // All's non-custom menu, do NOT return event's
        // they are handled by the system automatically
        MenuItem::About("Todos".to_string()),
        MenuItem::Services,
        MenuItem::Separator,
        MenuItem::Hide,
        MenuItem::HideOthers,
        MenuItem::ShowAll,
        MenuItem::Separator,
        quit_menu,
      ],
    ),
    Menu::new(
      "File",
      vec![
        custom_print_menu,
        MenuItem::Separator,
        other_test_menu,
        MenuItem::CloseWindow,
      ],
    ),
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
    Menu::new("View", vec![MenuItem::EnterFullScreen]),
    Menu::new("Window", vec![MenuItem::Minimize, MenuItem::Zoom]),
    Menu::new(
      "Help",
      vec![MenuItem::new("Custom help").with_accelerators("<Primary><Shift>h")],
    ),
  ];

  // Attention, Windows only support custom menu for now.
  // If we add any `MenuItem::*` they'll not render
  // We need to use custom menu with `Menu::new()` and catch
  // the events in the EventLoop.
  #[cfg(target_os = "windows")]
  let menu = vec![
    Menu::new("File", vec![other_test_menu]),
    Menu::new("Other menu", vec![quit_menu]),
  ];

  // Build our event loop
  let event_loop = EventLoop::new();
  // Build the window
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .with_menu(menu)
    .build(&event_loop)?;
  // Build the webview
  let webview = WebViewBuilder::new(window)?
    .with_url("https://tauri.studio")?
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
        // The custom menu expose an `id()` function to match with `menu_id` from the Event
        if menu_id == custom_print_menu_id {
          // the webview.print() is only working on macOS for now
          webview.print().expect("Unable to print");
        }
        if menu_id == quit_menu_id {
          // when we click on quit, let's close the app
          *control_flow = ControlFlow::Exit;
        }
        println!("Clicked on {:?}", menu_id);
      }
      _ => (),
    }
  });
}

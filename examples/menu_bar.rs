// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() -> wry::Result<()> {
  use wry::{
    application::{
      accelerator::{Accelerator, SysMods},
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      keyboard::KeyCode,
      menu::{MenuBar as Menu, MenuItem, MenuItemAttributes, MenuType},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };
  // Build our event loop
  let event_loop = EventLoop::new();

  // create main menubar menu
  let mut menu_bar_menu = Menu::new();

  // create `first_menu`
  let mut first_menu = Menu::new();

  // create second menu
  let mut second_menu = Menu::new();

  // create third menu
  let mut third_menu = Menu::new();

  // create an empty menu to be used as submenu
  let mut my_sub_menu = Menu::new();

  let mut print_item = my_sub_menu.add_item(
    MenuItemAttributes::new("Print")
      .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyP)),
  );

  first_menu.add_native_item(MenuItem::About("Todos".to_string()));
  first_menu.add_native_item(MenuItem::Services);
  first_menu.add_native_item(MenuItem::Separator);
  first_menu.add_native_item(MenuItem::Hide);
  first_menu.add_native_item(MenuItem::HideOthers);
  first_menu.add_native_item(MenuItem::ShowAll);
  let quit_item = first_menu.add_item(
    MenuItemAttributes::new("Quit")
      .with_accelerators(&Accelerator::new(SysMods::Cmd, KeyCode::KeyQ)),
  );

  third_menu.add_item(
    MenuItemAttributes::new("Custom help")
      .with_accelerators(&Accelerator::new(SysMods::CmdShift, KeyCode::KeyH)),
  );

  second_menu.add_submenu("Sub menu", true, my_sub_menu);
  second_menu.add_native_item(MenuItem::Copy);
  second_menu.add_native_item(MenuItem::Paste);
  second_menu.add_native_item(MenuItem::SelectAll);

  menu_bar_menu.add_submenu("First menu", true, first_menu);
  menu_bar_menu.add_submenu("Second menu", true, second_menu);
  menu_bar_menu.add_submenu("Help", true, third_menu);

  // Build the window
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .with_menu(menu_bar_menu)
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
        origin: MenuType::MenuBar,
        ..
      } => {
        // The custom menu expose an `id()` function to match with `menu_id` from the Event
        if menu_id == print_item.clone().id() {
          // the webview.print() is only working on macOS for now
          webview.print().expect("Unable to print");
          // limit print to a single-use
          print_item.set_enabled(false);
        }
        if menu_id == quit_item.clone().id() {
          // when we click on quit, let's close the app
          *control_flow = ControlFlow::Exit;
        }
        println!("Clicked on {:?}", menu_id);
      }
      _ => (),
    }
  });
}

#[cfg(target_os = "ios")]
fn main() {
  println!("This platform doesn't support menu_bar.");
}

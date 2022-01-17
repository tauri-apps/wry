// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[cfg(any(feature = "tray", feature = "ayatana"))]
fn main() -> wry::Result<()> {
  use std::collections::HashMap;
  #[cfg(target_os = "linux")]
  use std::path::Path;
  #[cfg(target_os = "macos")]
  use wry::application::platform::macos::{
    ActivationPolicy, CustomMenuItemExtMacOS, EventLoopExtMacOS, NativeImage,
  };
  #[cfg(target_os = "linux")]
  use wry::application::platform::unix::WindowBuilderExtUnix;
  #[cfg(target_os = "windows")]
  use wry::application::platform::windows::WindowBuilderExtWindows;
  use wry::{
    application::{
      accelerator::{Accelerator, SysMods},
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      global_shortcut::ShortcutManager,
      keyboard::KeyCode,
      menu::{ContextMenu, MenuItemAttributes, MenuType},
      system_tray::SystemTrayBuilder,
      window::{WindowBuilder, WindowId},
    },
    http::ResponseBuilder,
    webview::{WebView, WebViewBuilder},
  };

  let index_html = r#"
  <!DOCTYPE html>
  <html lang="en">
    <head>
      <meta charset="UTF-8" />
      <meta http-equiv="X-UA-Compatible" content="IE=edge" />
      <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    </head>
    <body>
      <h1>Welcome to WRY!</h1>
      <textarea style="width: 90vw; height: 30vh;"></textarea>
    </body>
  </html>"#;

  // Build our event loop
  #[cfg(target_os = "macos")]
  let mut event_loop = EventLoop::new();

  #[cfg(not(target_os = "macos"))]
  let event_loop = EventLoop::new();

  // launch macos app without menu and without dock icon
  // should be set at launch
  #[cfg(target_os = "macos")]
  event_loop.set_activation_policy(ActivationPolicy::Accessory);

  let mut webviews: HashMap<WindowId, WebView> = HashMap::new();

  // Create global shortcut
  let mut shortcut_manager = ShortcutManager::new(&event_loop);
  // SysMods::CmdShift; Command + Shift on macOS, Ctrl + Shift on windows/linux
  let my_accelerator = Accelerator::new(SysMods::CmdShift, KeyCode::Digit0);
  let global_shortcut = shortcut_manager.register(my_accelerator.clone()).unwrap();

  // Create sample menu item
  let mut tray_menu = ContextMenu::new();
  let open_new_window = tray_menu.add_item(MenuItemAttributes::new("Open new window"));
  // custom quit who take care to clean windows tray icon
  let quit_item = tray_menu.add_item(MenuItemAttributes::new("Quit"));

  // set NativeImage for `Open new window`
  #[cfg(target_os = "macos")]
  open_new_window
    .clone()
    .set_native_image(NativeImage::StatusAvailable);

  // Windows require Vec<u8> ICO file
  #[cfg(target_os = "windows")]
  let icon = include_bytes!("icon.ico").to_vec();
  // macOS require Vec<u8> PNG file
  #[cfg(target_os = "macos")]
  let icon = include_bytes!("icon.png").to_vec();
  // Linux require Pathbuf to PNG file
  #[cfg(target_os = "linux")]
  let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/icon.png");

  // Windows require Vec<u8> ICO file
  #[cfg(target_os = "windows")]
  let new_icon = include_bytes!("icon_blue.ico").to_vec();
  // macOS require Vec<u8> PNG file
  #[cfg(target_os = "macos")]
  let new_icon = include_bytes!("icon_dark.png").to_vec();
  // Linux require Pathbuf to PNG file
  #[cfg(target_os = "linux")]
  let new_icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/icon_dark.png");

  let mut system_tray = SystemTrayBuilder::new(icon.clone(), Some(tray_menu))
    .build(&event_loop)
    .unwrap();

  // launch WRY process
  event_loop.run(move |event, event_loop, control_flow| {
    *control_flow = ControlFlow::Wait;

    let mut create_window_or_focus = || {
      // if we already have one webview, let's focus instead of opening
      if !webviews.is_empty() {
        for window in webviews.values() {
          window.window().set_focus();
        }
        return;
      }

      // disable our global shortcut
      shortcut_manager
        .unregister(global_shortcut.clone())
        .unwrap();

      // create our new window / webview instance
      #[cfg(any(target_os = "windows", target_os = "linux"))]
      let window_builder = WindowBuilder::new().with_skip_taskbar(true);
      #[cfg(target_os = "macos")]
      let window_builder = WindowBuilder::new();

      let window = window_builder.build(event_loop).unwrap();

      let id = window.id();

      let webview = WebViewBuilder::new(window)
        .unwrap()
        .with_custom_protocol("wry.dev".into(), move |_request| {
          ResponseBuilder::new()
            .mimetype("text/html")
            .body(index_html.as_bytes().into())
        })
        .with_url("wry.dev://")
        .unwrap()
        .build()
        .unwrap();

      webviews.insert(id, webview);

      // make sure open_new_window is mutable
      let mut open_new_window = open_new_window.clone();
      // disable button
      open_new_window.set_enabled(false);
      // change title (text)
      open_new_window.set_title("Window already open");
      // set checked
      open_new_window.set_selected(true);
      // update tray i  con
      system_tray.set_icon(new_icon.clone());
      // add macOS Native red dot
      #[cfg(target_os = "macos")]
      open_new_window.set_native_image(NativeImage::StatusUnavailable);
    };

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        window_id,
        ..
      } => {
        println!("Window {:?} has received the signal to close", window_id);
        let mut open_new_window = open_new_window.clone();
        // Remove window from our hashmap
        webviews.remove(&window_id);
        // Modify our button's state
        open_new_window.set_enabled(true);
        // Reset text
        open_new_window.set_title("Open new window");
        // Set selected
        open_new_window.set_selected(false);
        // Change tray icon
        system_tray.set_icon(icon.clone());
        // re-active our global shortcut
        shortcut_manager.register(my_accelerator.clone()).unwrap();
        // macOS have native image available that we can use in our menu-items
        #[cfg(target_os = "macos")]
        open_new_window.set_native_image(NativeImage::StatusAvailable);
      }
      // on Windows, habitually, we show the Window with left click
      // and the menu is shown on right click
      #[cfg(target_os = "windows")]
      Event::TrayEvent {
        event: tao::event::TrayEvent::LeftClick,
        ..
      } => create_window_or_focus(),
      // Catch menu events
      Event::MenuEvent {
        menu_id,
        // specify only context menu's
        origin: MenuType::ContextMenu,
        ..
      } => {
        // Click on Open new window or focus item
        if menu_id == open_new_window.clone().id() {
          create_window_or_focus();
        }
        // click on `quit` item
        if menu_id == quit_item.clone().id() {
          // tell our app to close at the end of the loop.
          *control_flow = ControlFlow::Exit;
        }
        println!("Clicked on {:?}", menu_id);
      }
      // catch global shortcut event and open window
      Event::GlobalShortcutEvent(hotkey_id) if hotkey_id == my_accelerator.clone().id() => {
        create_window_or_focus()
      }
      _ => (),
    }
  });
}

#[cfg(target_os = "ios")]
fn main() {
  println!("This platform doesn't support system_tray.");
}

// Tray feature flag disabled but can be available.
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
#[cfg(not(any(feature = "tray", feature = "ayatana")))]
fn main() {
  println!("This platform doesn't have the `tray` feature enabled.");
}

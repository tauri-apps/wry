// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[cfg(any(feature = "tray", feature = "ayatana"))]
fn main() -> wry::Result<()> {
  use std::collections::HashMap;
  #[cfg(target_os = "linux")]
  use std::path::Path;
  #[cfg(target_os = "linux")]
  use tao::menu::{ContextMenu, MenuItemAttributes};
  #[cfg(target_os = "macos")]
  use wry::application::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
  #[cfg(target_os = "linux")]
  use wry::application::platform::unix::WindowBuilderExtUnix;
  #[cfg(target_os = "windows")]
  use wry::application::platform::windows::WindowBuilderExtWindows;
  use wry::{
    application::{
      dpi::{LogicalSize, PhysicalPosition},
      event::{Event, Rectangle, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      menu::MenuId,
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

  // Windows require Vec<u8> ICO file
  #[cfg(target_os = "windows")]
  let icon = include_bytes!("icon.ico").to_vec();
  // macOS require Vec<u8> PNG file
  #[cfg(target_os = "macos")]
  let icon = include_bytes!("icon.png").to_vec();
  // Linux require Pathbuf to PNG file
  #[cfg(target_os = "linux")]
  let icon = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/icon.png");

  // linux require a menu so let's add only a open button
  let open_menu_id = MenuId::new("open_menu");
  let quit_menu_id = MenuId::new("quit_menu");
  #[cfg(target_os = "linux")]
  {
    let mut menu = ContextMenu::new();
    menu.add_item(MenuItemAttributes::new("Open").with_id(open_menu_id));
    menu.add_item(MenuItemAttributes::new("Quit").with_id(quit_menu_id));
    let _system_tray = SystemTrayBuilder::new(icon, Some(menu))
      .build(&event_loop)
      .unwrap();
  }

  #[cfg(any(target_os = "macos", target_os = "windows"))]
  let _system_tray = SystemTrayBuilder::new(icon, None)
    .build(&event_loop)
    .unwrap();

  // little helper to position our window
  // centered with tray bounds
  fn window_position_center_tray(
    rectangle: &mut Rectangle,
    window_size: LogicalSize<f64>,
  ) -> PhysicalPosition<f64> {
    // center X axis with tray icon position
    let window_x =
      rectangle.position.x + ((rectangle.size.width / 2.0) - (window_size.width / 2.0));
    rectangle.position.x = window_x;

    // position Y axis (Windows only)
    #[cfg(target_os = "windows")]
    {
      rectangle.position.y = rectangle.position.y - window_size.height - rectangle.size.height;
    }

    (*rectangle).position
  }

  // launch WRY process
  event_loop.run(move |event, event_loop, control_flow| {
    *control_flow = ControlFlow::Wait;

    let mut create_window_or_focus =
      |window_size: LogicalSize<f64>, window_position: PhysicalPosition<f64>| {
        // if we already have one webview, let's focus instead of opening
        if !webviews.is_empty() {
          for window in webviews.values() {
            window.window().set_focus();
          }
          return;
        }

        // create our new window / webview instance
        let mut window_builder = WindowBuilder::new();
        window_builder = window_builder
          .with_position(window_position)
          .with_inner_size(window_size);

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
          window_builder = window_builder.with_skip_taskbar(true);
        }

        let window = window_builder.build(event_loop).unwrap();

        let id = window.id();

        let webview = WebViewBuilder::new(window)
          .unwrap()
          .with_custom_protocol("wry.dev".into(), move |_uri| {
            ResponseBuilder::new()
              .mimetype("text/html")
              .body(index_html.as_bytes().into())
          })
          .with_url("wry.dev://")
          .unwrap()
          .build()
          .unwrap();

        webviews.insert(id, webview);
      };

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        window_id,
        ..
      } => {
        println!("Window {:?} has received the signal to close", window_id);
        // Remove webview from our hashmap
        webviews.remove(&window_id);
      }
      // open app on left click centered with traty icon bound
      Event::TrayEvent {
        event: tao::event::TrayEvent::LeftClick,
        mut bounds,
        ..
      } => {
        let window_inner_size = LogicalSize::new(200.0, 200.0);
        let position = window_position_center_tray(&mut bounds, window_inner_size);
        create_window_or_focus(window_inner_size, position);
      }
      // open new window (linux)
      Event::MenuEvent { menu_id, .. } if menu_id == open_menu_id => {
        let window_inner_size = LogicalSize::new(200.0, 200.0);
        let position = PhysicalPosition::new(450.0, 450.0);
        create_window_or_focus(window_inner_size, position);
      }
      // request to quit (linux)
      Event::MenuEvent { menu_id, .. } if menu_id == quit_menu_id => {
        *control_flow = ControlFlow::Exit
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
  println!("This platform doesn't have the `tray` or `ayatana` feature enabled.");
}

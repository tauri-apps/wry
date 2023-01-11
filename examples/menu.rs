// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

const PAGE1_HTML: &[u8] = include_bytes!("custom_protocol_page1.html");

fn main() -> wry::Result<()> {
  use std::{
    fs::{canonicalize, read},
    path::PathBuf,
  };

  use wry::{
    application::{
      accelerator::Accelerator,
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      keyboard::{KeyCode, ModifiersState},
      menu::{MenuBar, MenuItemAttributes},
      window::WindowBuilder,
    },
    http::{header::CONTENT_TYPE, Response},
    webview::WebViewBuilder,
  };

  let mut menu = MenuBar::new();
  let mut file_menu = MenuBar::new();
  file_menu.add_native_item(tao::menu::MenuItem::Cut);
  file_menu.add_native_item(tao::menu::MenuItem::Copy);
  file_menu.add_native_item(tao::menu::MenuItem::Paste);
  file_menu.add_item(
    MenuItemAttributes::new("Quit").with_accelerators(&Accelerator::new(
      Some(ModifiersState::CONTROL | ModifiersState::SHIFT),
      KeyCode::KeyQ,
    )),
  );
  file_menu.add_item(
    MenuItemAttributes::new("Quit").with_accelerators(&Accelerator::new(None, KeyCode::KeyQ)),
  );
  file_menu.add_item(
    MenuItemAttributes::new("Quit").with_accelerators(&Accelerator::new(
      Some(ModifiersState::SHIFT),
      KeyCode::KeyQ,
    )),
  );
  menu.add_submenu("File", true, file_menu);

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Custom Protocol")
    .with_menu(menu)
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry".into(), move |request| {
      let path = request.uri().path();
      // Read the file content from file path
      let content = if path == "/" {
        PAGE1_HTML.into()
      } else {
        // `1..` for removing leading slash
        read(canonicalize(PathBuf::from("examples").join(&path[1..]))?)?.into()
      };

      // Return asset contents and mime types based on file extentions
      // If you don't want to do this manually, there are some crates for you.
      // Such as `infer` and `mime_guess`.
      let (data, meta) = if path.ends_with(".html") || path == "/" {
        (content, "text/html")
      } else if path.ends_with(".js") {
        (content, "text/javascript")
      } else if path.ends_with(".png") {
        (content, "image/png")
      } else if path.ends_with(".wasm") {
        (content, "application/wasm")
      } else {
        unimplemented!();
      };

      Response::builder()
        .header(CONTENT_TYPE, meta)
        .body(data)
        .map_err(Into::into)
    })
    // tell the webview to load the custom protocol
    .with_url("wry://localhost")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::MenuEvent { menu_id, .. } => {
        println!("Menu clicked! {:?}", menu_id);
        // *control_flow = ControlFlow::Exit;
      }
      _ => (),
    }
  });
}

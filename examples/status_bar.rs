// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

/*
  macOS:

  If your app is menu bar ONLY, similar to the Dropbox macOS client,
  you need to set Application is agent = YES in your Info.plist in order
  to omit the Dock's app icon and the app menu on the upper left corner.
*/

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  let status_bar_icon = include_bytes!("static/tauri_logo.png");

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_initialization_script("menacing = 'ゴ';")
    .with_status_bar("My app", status_bar_icon)
    .with_url("https://tauri.studio")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Poll;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => {
        // todo(lemarier): make sure this is the right way to do it
        // when we request the close from the `X`, we hide the window
        // so the status bar button will only set_visible to true
        webview.window().set_visible(false);

        // todo(lemarier): Once custom menu is implemented, we should
        // be able to update our menu bar items. By example, we could remove the
        // `hide` and put `show` with the right calback
      }
      _ => (),
    }
  });
}

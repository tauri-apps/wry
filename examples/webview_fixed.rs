// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
    use wry::{
      application::{
        event::{Event, StartCause, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
      },
      webview::WebViewBuilder,
    };
    use std::env;

    // env::set_var creates a temporary environment variable while to application is opened
    // set here the path to the unzipped .cab file, which contains the fixed version of the webview runtime
    env::set_var(
        "WEBVIEW2_BROWSER_EXECUTABLE_FOLDER",
        "<path_to_your_webview_director>",
    );
  
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
      .with_title("Hello World")
      .build(&event_loop)?;
    let _webview = WebViewBuilder::new(window)?
      .with_url("https://tauri.studio")?
      .build()?;
  
    event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Poll;
  
      match event {
        Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => *control_flow = ControlFlow::Exit,
        _ => (),
      }
    });
  }
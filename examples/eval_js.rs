// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoopBuilder},
      window::{Window, WindowBuilder},
    },
    webview::WebViewBuilder,
  };

  enum UserEvent {
    ExecEval,
  }

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();

  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)?;

  let ipc_handler = move |_: &Window, req: String| {
    if req == "exec-eval" {
      let _ = proxy.send_event(UserEvent::ExecEval);
    }
  };

  let _webview = WebViewBuilder::new(window)?
    .with_html(
      r#"
      <button onclick="window.ipc.postMessage('exec-eval')">Exec eval</button>
    "#,
    )?
    .with_ipc_handler(ipc_handler)
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::UserEvent(UserEvent::ExecEval) => {
        // String
        _webview
          .evaluate_script_with_callback(
            "if (!foo) { var foo = 'morbin'; } `${foo} time`",
            |result| println!("String: {:?}", result),
          )
          .unwrap();

        // Number
        _webview
          .evaluate_script_with_callback("var num = 9527; num", |result| {
            println!("Number: {:?}", result)
          })
          .unwrap();

        // Object
        _webview
          .evaluate_script_with_callback("var obj = { thank: 'you', '95': 27 }; obj", |result| {
            println!("Object: {:?}", result)
          })
          .unwrap();

        // Array
        _webview
          .evaluate_script_with_callback("var ary = [1,2,3,4,'5']; ary", |result| {
            println!("Array: {:?}", result)
          })
          .unwrap();
        // Exception thrown
        _webview
          .evaluate_script_with_callback("throw new Error()", |result| {
            println!("Exception Occured: {:?}", result)
          })
          .unwrap();
      }
      Event::NewEvents(StartCause::Init) => println!("Wry has started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

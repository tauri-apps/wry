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

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry.dev".into(), move |_, requested_asset_path| {
      // remove the protocol from the path for easiest match
      let requested_asset_path = requested_asset_path.replace("wry.dev://", "");

      // sample index.html file
      // files can be bundled easilly into the binary
      // with https://doc.rust-lang.org/std/macro.include_bytes.html

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
          <a href="/hello.html">Link</a>
          <script type="text/javascript" src="/hello.js"></script>
        </body>
      </html>"#;

      // sample hello.js file
      let hello_js = "console.log(\"hello from javascript\");";

      // sample hello.html file
      let hello_html = r#"
      <!DOCTYPE html>
      <html lang="en">
        <head>
          <meta charset="UTF-8" />
          <meta http-equiv="X-UA-Compatible" content="IE=edge" />
          <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        </head>
        <body>
          <h1>Sample page!</h1>
          <a href="/index.html">Back home</a>
          <script type="text/javascript" src="/hello.js"></script>
        </body>
      </html>"#;

      match requested_asset_path.as_str() {
        // if our path match /hello.html
        "/hello.html" => Ok(hello_html.as_bytes().into()),
        // if our path match /hello.js
        "/hello.js" => Ok(hello_js.as_bytes().into()),
        // other paths should resolve index
        // more logic can be applied here
        _ => Ok(index_html.as_bytes().into()),
      }
    })
    // tell the webview to load the custom protocol
    .with_url("wry.dev://")?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

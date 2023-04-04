// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
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

    let html = r#"
      <!DOCTYPE html>
      <html>
        <head>
          
        </head>
        <body>
        <h1>Your cookie is: </h1>
        <p id="cookie"></p>
        <button onclick="createCookie()">Create cookie</button>
        <button onclick="getCookie()">Get cookie</button>
      </body>
      <script>
          let cookie = document.getElementById("cookie");
          function createCookie() {
            let date = new Date();
            let time = date.getTime();
            date.setTime(time + 999999);
            let rand = Math.random();
            let c = `token=${rand}`;
            document.cookie = `token=${rand};expires=${date.toUTCString()}`;
            cookie.innerHTML = document.cookie;
          }
  
          function getCookie() {
            cookie.innerHTML = document.cookie;
          }
  
          getCookie();
          </script>  
      </html>
    "#;
    
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
      .with_title("Hello World")
      .build(&event_loop)?;
    let _webview = WebViewBuilder::new(window)?
      .with_html(html)?
      .as_incognito(false)
      .build()?;
  
    event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;
  
      match event {
        Event::NewEvents(StartCause::Init) => {
          println!("Wry has started!");
        },
        Event::WindowEvent {
          event: WindowEvent::CloseRequested,
          ..
        } => *control_flow = ControlFlow::Exit,
        _ => (),
      }
    });
  }
  
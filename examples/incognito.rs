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
  use http::Response;
  use std::borrow::Cow;

  let html = r#"
      <!DOCTYPE html>
      <html>
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
    .with_title("WRY Incognito!")
    .build(&event_loop)?;
  
  // Use custom protocol as a workaround for example issues with Windows
  #[cfg(windows)]
  let _webview = WebViewBuilder::new(window)?
    .as_incognito(false)
    .with_custom_protocol("incognito".to_owned(), |_req| {
      let cow = Cow::from(html.as_bytes());
      Ok(Response::builder().body(cow).unwrap())
    }).with_url("https://incognito.localhost")?.build()?;

  // Use the HTML directly for non-Windows OSes
  #[cfg(not(windows))]
  let _webview = WebViewBuilder::new(window)?
    .as_incognito(false)
    .with_html(html)?.build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

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

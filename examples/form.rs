// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  borrow::Cow,
  fs::{canonicalize, read},
};

use wry::{
  application::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
  },
  http::{header::CONTENT_TYPE, method::Method, Response},
  webview::WebViewBuilder,
};

fn main() -> wry::Result<()> {
  let event_loop = EventLoop::new();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry".into(), move |request| {
      if request.method() == Method::POST {
        let body_string = String::from_utf8_lossy(request.body());
        for body in body_string.split('&') {
          println!("Value sent; {:?}", body);
        }
      }

      // remove leading slash
      let path = &request.uri().path()[1..];

      get_response(path).unwrap_or_else(|error| {
        http::Response::builder()
          .status(http::StatusCode::BAD_REQUEST)
          .header(CONTENT_TYPE, "text/plain")
          .body(error.to_string().as_bytes().to_vec().into())
          .unwrap()
      })
    })
    // tell the webview to load the custom protocol
    .with_url("wry://localhost/examples/form.html")?
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

fn get_response(path: &str) -> Result<Response<Cow<'static, [u8]>>, Box<dyn std::error::Error>> {
  Response::builder()
    .header(CONTENT_TYPE, "text/html")
    .body(read(canonicalize(path)?)?.into())
    .map_err(Into::into)
}

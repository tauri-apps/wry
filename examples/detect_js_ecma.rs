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
    .with_title("Detect ECMAScript")
    .build(&event_loop)
    .unwrap();
  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_initialization_script(
    r#"
        (function () {
            window.addEventListener('DOMContentLoaded', (event) => {
                function addEntry(title, available) {
                    document.getElementById("table").innerHTML += `<tr> <td style="width: 200px">${title}</td> <td>${available ? '✔' : '❌'} </td> </tr>`
                }

                addEntry("ECMAScript 5 (2009)", Array.isArray)
                addEntry("ECMAScript 6 (2015)", Array.prototype.find)
                addEntry("ECMAScript 2016", Array.prototype.includes)
                addEntry("ECMAScript 2017", Object.entries)
                addEntry("ECMAScript 2018", Promise.prototype.finally)
                addEntry("ECMAScript 2019", Object.fromEntries)
                addEntry("ECMAScript 2020", BigInt)
                addEntry("ECMAScript 2021", WeakRef)
            });
        })();
        "#)
    .with_url(
    r#"data:text/html,
    </html>
        <body>
            <table>
                <thead>
                    <h3>ECMAScript support list:<h3>
                </thead>
                <tbody id="table"></tbody>
            </table>
        </body>
    </html>
    "#,
    )?
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Poll;

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

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
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
    CloseWindow,
  }

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let window = WindowBuilder::new()
    .with_decorations(false)
    .build(&event_loop)
    .unwrap();

  const HTML: &str = r#"
  <html>

  <head>
      <style>
          html {
            font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
          }

          * {
              padding: 0;
              margin: 0;
              box-sizing: border-box;
          }

          main {
            display: grid;
            place-items: center;
            height: calc(100vh - 30px);
          }

          .titlebar {
              height: 30px;
              padding-left: 5px;
              display: grid;
              grid-auto-flow: column;
              grid-template-columns: 1fr max-content max-content max-content;
              align-items: center;
              background: #1F1F1F;
              color: white;
              user-select: none;
          }

          .titlebar-button {
              display: inline-flex;
              justify-content: center;
              align-items: center;
              width: 30px;
              height: 30px;
          }

          .titlebar-button:hover {
              background: #3b3b3b;
          }

          .titlebar-button#close:hover {
              background: #da3d3d;
          }

          .titlebar-button img {
              filter: invert(100%);
          }
      </style>
  </head>

  <body>
      <div class="titlebar">
          <div class="drag-region">Custom Titlebar</div>
          <div>
              <div class="titlebar-button" onclick="window.ipc.postMessage('minimize')">
                  <img src="https://api.iconify.design/codicon:chrome-minimize.svg" />
              </div>
              <div class="titlebar-button" onclick="window.ipc.postMessage('maximize')">
                  <img src="https://api.iconify.design/codicon:chrome-maximize.svg" />
              </div>
              <div class="titlebar-button" id="close" onclick="window.ipc.postMessage('close')">
                  <img src="https://api.iconify.design/codicon:close.svg" />
              </div>
          </div>
      </div>
      <main>
          <h4> WRYYYYYYYYYYYYYYYYYYYYYY! </h4>
      </main>
      <script>
          document.addEventListener('mousedown', (e) => {
              if (e.target.classList.contains('drag-region') && e.buttons === 1) {
                  e.detail === 2
                      ? window.ipc.postMessage('maximize')
                      : window.ipc.postMessage('drag_window');
              }
          })
          document.addEventListener('touchstart', (e) => {
              if (e.target.classList.contains('drag-region')) {
                  window.ipc.postMessage('drag_window');
              }
          })
      </script>
  </body>

  </html>
"#;

  let proxy = event_loop.create_proxy();

  let handler = move |window: &Window, req: String| {
    if req == "minimize" {
      window.set_minimized(true);
    }
    if req == "maximize" {
      window.set_maximized(!window.is_maximized());
    }
    if req == "close" {
      let _ = proxy.send_event(UserEvent::CloseWindow);
    }
    if req == "drag_window" {
      let _ = window.drag_window();
    }
  };

  let mut webview = Some(
    WebViewBuilder::new(window)
      .unwrap()
      .with_html(HTML)?
      .with_ipc_handler(handler)
      .with_accept_first_mouse(true)
      .build()?,
  );

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => println!("Wry application started!"),
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      }
      | Event::UserEvent(UserEvent::CloseWindow) => {
        let _ = webview.take();
        *control_flow = ControlFlow::Exit
      }
      _ => (),
    }
  });
}

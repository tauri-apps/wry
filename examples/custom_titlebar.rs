use tao::window::WindowId;

// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::{Window, WindowBuilder},
    },
    webview::WebViewBuilder,
  };

  enum UserEvents {
    CloseWindow(WindowId),
  }

  let event_loop = EventLoop::<UserEvents>::with_user_event();
  let mut webviews = std::collections::HashMap::new();
  let window = WindowBuilder::new()
    .with_decorations(false)
    .build(&event_loop)
    .unwrap();

  let url = r#"data:text/html,
        <body>
          <div class='drag-region titlebar'>
            <div class="left">Awesome WRY Window</div>
            <div class="right">
              <div class="titlebar-button" id="minimize">
                <img src="https://api.iconify.design/codicon:chrome-minimize.svg" />
              </div>
              <div class="titlebar-button" id="maximize">
                <img src="https://api.iconify.design/codicon:chrome-maximize.svg" />
              </div>
              <div class="titlebar-button" id="close">
                <img src="https://api.iconify.design/codicon:close.svg" />
              </div>
            </div>
          </div>
          <div>
            <p> WRYYYYYYYYYYYYYYYYYYYYYY! </p>
            <button style="cursor: pointer"> Hover me! </button>
          </div>
        </body>
      "#;

  let script = r#"
  (function () {
    window.addEventListener('DOMContentLoaded', (event) => {
      document.getElementById('minimize').addEventListener('click', () => ipc.postMessage('minimize'));
      document.getElementById('maximize').addEventListener('click', () => ipc.postMessage('maximize'));
      document.getElementById('close').addEventListener('click', () => ipc.postMessage('close'));

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

      const style = document.createElement('style');
      style.textContent = `
        * {
          padding: 0;
          margin: 0;
          box-sizing: border-box;
        }
        .titlebar {
          height: 30px;
          background: #1F1F1F;
          color: white;
          user-select: none;
          display: flex;
          justify-content: space-between;
          align-items: center;
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
        .titlebar-button:nth-child(3):hover {
          background: #da3d3d;
        }
        .titlebar-button img {
          filter: invert(100%);
        }
      `;
      document.head.append(style);
    });
  })();
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
      let _ = proxy.send_event(UserEvents::CloseWindow(window.id()));
    }
    if req == "drag_window" {
      let _ = window.drag_window();
    }
  };

  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url(url)?
    .with_initialization_script(script)
    .with_ipc_handler(handler)
    .build()?;
  webviews.insert(webview.window().id(), webview);

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::WindowEvent {
        event, window_id, ..
      } => match event {
        WindowEvent::CloseRequested => {
          webviews.remove(&window_id);
          if webviews.is_empty() {
            *control_flow = ControlFlow::Exit
          }
        }
        WindowEvent::Resized(_) => {
          let _ = webviews[&window_id].resize();
        }
        _ => (),
      },
      Event::UserEvent(UserEvents::CloseWindow(id)) => {
        webviews.remove(&id);
        if webviews.is_empty() {
          *control_flow = ControlFlow::Exit
        }
      }
      _ => (),
    }
  });
}

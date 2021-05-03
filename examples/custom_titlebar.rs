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
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
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
            WRYYYYYYYYYYYYYYYYYYYYYY!
          </div>
        </body>
      "#;

  let script = r#"
  (function () {
    window.addEventListener('DOMContentLoaded', (event) => {
      document.getElementById('minimize').addEventListener('click', () => rpc.notify('minimize'));
      document.getElementById('maximize').addEventListener('click', () => rpc.notify('maximize'));
      document.getElementById('close').addEventListener('click', () => rpc.notify('close'));

      document.addEventListener('mousedown', (e) => {
        if (e.target.classList.contains('drag-region') && e.buttons === 1) {
          window.rpc.notify('drag_window');
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

  let (window_tx, window_rx) = std::sync::mpsc::channel();

  let handler = move |window: &Window, req: RpcRequest| {
    if req.method == "minimize" {
      window.set_minimized(true);
    }
    if req.method == "maximize" {
      if window.is_maximized() {
        window.set_maximized(false);
      } else {
        window.set_maximized(true);
      }
    }
    if req.method == "close" {
      let _ = window_tx.send(window.id());
    }
    if req.method == "drag_window" {
      let _ = window.drag_window();
    }
    None
  };

  let webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url(url)?
    .with_initialization_script(script)
    .with_rpc_handler(handler)
    .build()?;
  webviews.insert(webview.window().id(), webview);

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    if let Ok(id) = window_rx.try_recv() {
      webviews.remove(&id);
      if webviews.is_empty() {
        *control_flow = ControlFlow::Exit
      }
    }

    if let Event::WindowEvent { event, window_id } = event {
      match event {
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
      }
    }
  });
}

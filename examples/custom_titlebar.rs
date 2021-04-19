// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::{RpcRequest, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
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
        <script>
          let maximized = false;
          document.getElementById('minimize').addEventListener('click', () => rpc.notify('minimize'));
          document.getElementById('maximize').addEventListener('click', () => {
            maximized = !maximized;
            rpc.notify('maximize', maximized);
          });
          document.getElementById('close').addEventListener('click', () => rpc.notify('close'));
        </script>
      "#;

  let handler = |mut _req: RpcRequest| {
    /* TODO window setter
    if req.method == "minimize" {
      proxy.minimize().unwrap();
    }
    if req.method == "maximize" {
      if req.params.unwrap().as_array().unwrap()[0] == true {
        proxy.maximize().unwrap();
      } else {
        proxy.unmaximize().unwrap();
      }
    }
    if req.method == "close" {
      proxy.close().unwrap();
    }
    */
    None
  };
  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url(url)?
    .with_rpc_handler(handler)
    // inject the css after 500ms, otherwise it won't work as the `head` element isn't created yet.
    .with_initialization_script(
      r#"
        setTimeout(() => {
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
        }, 500);
      "#,
    )
    .build()?;

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Poll;

    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ => (),
    }
  });
}

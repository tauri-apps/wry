// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use wry::{Application, Attributes, Result, RpcRequest, WindowProxy, WindowRpcHandler};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  let attributes = Attributes {
    url: Some(
      r#"data:text/html,
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
      "#.into(),
    ),
    // inject the css after 500ms, otherwise it won't work as the `head` element isn't created yet.
    initialization_scripts:vec![
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
      "#.into()],
    decorations: false,
    ..Default::default()
  };

  let handler: WindowRpcHandler = Box::new(|proxy: WindowProxy, req: RpcRequest| {
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
    None
  });

  let _window1 = app.add_window_with_configs(attributes, Some(handler), vec![], None)?;

  app.run();
  Ok(())
}

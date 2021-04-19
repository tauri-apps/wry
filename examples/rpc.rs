// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize)]
struct MessageParameters {
  message: String,
}

fn main() -> wry::Result<()> {
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::{RpcRequest, RpcResponse, WebViewBuilder},
  };

  let event_loop = EventLoop::new();
  let window = WindowBuilder::new().build(&event_loop).unwrap();

  let url = r#"data:text/html,
<script>
let fullscreen = false;
async function toggleFullScreen() {
    await rpc.call('fullscreen', !fullscreen);
    fullscreen = !fullscreen;
}

async function getAsyncRpcResult() {
    const reply = await rpc.call('send-parameters', {'message': 'WRY'});
    const result = document.getElementById('rpc-result');
    result.innerText = reply;
}

</script>
<div><button onclick="toggleFullScreen();">Toggle fullscreen</button></div>
<div><button onclick="getAsyncRpcResult();">Send parameters</button></div>
<div id="rpc-result"></div>
"#;

  let handler = |mut req: RpcRequest| {
    let mut response = None;
    if &req.method == "fullscreen" {
      if let Some(params) = req.params.take() {
        if let Some(mut args) = serde_json::from_value::<Vec<bool>>(params).ok() {
          if args.len() > 0 {
            let _flag = args.swap_remove(0);
            // NOTE: in the real world we need to reply with an error
            // TODO let _ = window.set_fullscreen(flag);
          };
          response = Some(RpcResponse::new_result(req.id.take(), None));
        }
      }
    } else if &req.method == "send-parameters" {
      if let Some(params) = req.params.take() {
        if let Some(mut args) = serde_json::from_value::<Vec<MessageParameters>>(params).ok() {
          let result = if args.len() > 0 {
            let msg = args.swap_remove(0);
            Some(Value::String(format!("Hello, {}!", msg.message)))
          } else {
            // NOTE: in the real-world we should send an error response here!
            None
          };
          // Must always send a response as this is a `call()`
          response = Some(RpcResponse::new_result(req.id.take(), result));
        }
      }
    }

    response
  };
  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_url(url)?
    .with_rpc_handler(handler)
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

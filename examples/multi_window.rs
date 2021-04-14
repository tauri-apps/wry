// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use serde_json::Value;
use wry::{Application, Attributes, Result, RpcRequest, WindowProxy};

fn main() -> Result<()> {
  let mut app = Application::new()?;

  let attributes = Attributes {
    url: Some(format!("https://tauri.studio")),
    // Initialization scripts can be used to define javascript functions and variables.
    initialization_scripts: vec![r#"async function openWindow() {
                await window.rpc.notify("openWindow", "https://i.imgur.com/x6tXcr9.gif");
            }"#
      .to_string()],
    ..Default::default()
  };

  let (window_tx, window_rx) = std::sync::mpsc::channel::<String>();
  let handler = Box::new(move |_: WindowProxy, req: RpcRequest| {
    if &req.method == "openWindow" {
      if let Some(params) = req.params {
        if let Value::String(url) = &params[0] {
          let _ = window_tx.send(url.to_string());
        }
      }
    }
    None
  });

  let window_proxy = app.add_window_with_configs(attributes, Some(handler), vec![], None)?;
  let app_proxy = app.application_proxy();
  std::thread::spawn(move || {
    let mut count = 1;
    loop {
      if let Ok(url) = window_rx.try_recv() {
        let new_window = app_proxy
          .add_window(Attributes {
            width: 426.,
            height: 197.,
            title: "RODA RORA DA".into(),
            url: Some(url),
            ..Default::default()
          })
          .unwrap();
        println!("ID of new window: {:?}", new_window.id());
      } else if count < 8 {
        println!("{} seconds have passed...", count);
        count += 1;
      } else if count == 8 {
        window_proxy.evaluate_script("openWindow()").unwrap();
        count += 1;
      }
      std::thread::sleep(std::time::Duration::new(1, 0));
    }
  });

  app.run();
  Ok(())
}

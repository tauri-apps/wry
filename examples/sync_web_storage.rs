// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::rc::Rc;

#[cfg(target_os = "macos")]
use cocoa::base::id;
#[cfg(target_os = "macos")]
use objc::{msg_send, class, sel, sel_impl};

fn main() -> wry::Result<()> {
  use std::collections::HashMap;
  use wry::{
    application::{
      event::{Event, WindowEvent},
      event_loop::{ControlFlow, EventLoop, EventLoopProxy, EventLoopWindowTarget},
      window::{Window, WindowBuilder, WindowId},
    },
    webview::{WebView, WebViewBuilder},
  };

  enum UserEvents {
    CloseWindow(WindowId),
    NewWindow(),
  }

  #[cfg(target_os = "macos")]
  let process_pool: Rc<id> = Rc::new(unsafe {
    msg_send![class!(WKProcessPool), new]
  });

  fn create_new_window(
    title: String,
    event_loop: &EventLoopWindowTarget<UserEvents>,
    proxy: EventLoopProxy<UserEvents>,
    #[cfg(target_os = "macos")]
    process_pool: Rc<id>
  ) -> (WindowId, WebView) {
    let window = WindowBuilder::new()
      .with_title(title)
      .build(event_loop)
      .unwrap();
    let window_id = window.id();
    let handler = move |window: &Window, req: String| match req.as_str() {
      "new-window" => {
        let _ = proxy.send_event(UserEvents::NewWindow());
      }
      "close" => {
        let _ = proxy.send_event(UserEvents::CloseWindow(window.id()));
      }
      _ if req.starts_with("change-title") => {
        let title = req.replace("change-title:", "");
        window.set_title(title.as_str());
      }
      _ => {}
    };

    let webview = WebViewBuilder::new(window)
      .unwrap()
      .with_devtools(true)
      .with_html(
        r#"
          <button onclick="window.ipc.postMessage('new-window')">Open a new window</button>
          <button onclick="window.ipc.postMessage('close')">Close current window</button>
          <input oninput="window.ipc.postMessage(`change-title:${this.value}`)" />
          <br/>
          <button id="write-cookie-button">write cookie</button>
          <input placeholder="key=value" id="write-cookie-value" />
          <span id="write-cookie-result"></span>
          <br/>
          <button id="read-cookie-button">read cookie</button>
          <input placeholder="key" id="read-cookie-value" />
          <span id="read-cookie-result"></span>

          <script>
            const writeCookieButton = document.getElementById("write-cookie-button");
            writeCookieButton.addEventListener("click", () => {
              const cookie = document.getElementById("write-cookie-value").value;
              // If you use this code in production, you need to check cookie value for preventing XSS.
              // document.cookie = cookie;
              localStorage.setItem(...cookie.split("="));
              const result = document.getElementById("write-cookie-result");
              result.textContent = `Writing ${cookie} was successful!`
            });

            const readCookieButton = document.getElementById("read-cookie-button");
            readCookieButton.addEventListener("click", () => {
              const cookieKey = document.getElementById("read-cookie-value").value;
              // If you use this code in production, you need to check cookie value for preventing XSS.
              // const cookieValue = document.cookie.split(";").find((row) => row.startsWith(`${cookieKey}=`))?.split('=')[1];
              const cookieValue = localStorage.getItem(cookieKey);
              const result = document.getElementById("read-cookie-result");
              result.textContent = `result: ${cookieKey}=${cookieValue}`
            });
          </script>
      "#,
      )
      .unwrap()
      .with_ipc_handler(handler);

    #[cfg(target_os = "macos")]
    let webview = webview.with_process_pool(*process_pool);

    let webview = webview.build().unwrap();

    (window_id, webview)
  }

  let event_loop = EventLoop::<UserEvents>::with_user_event();
  let mut webviews = HashMap::new();
  let proxy = event_loop.create_proxy();

  let new_window = create_new_window(
    format!("Window {}", webviews.len() + 1),
    &event_loop,
    proxy.clone(),
    #[cfg(target_os = "macos")]
    process_pool.clone()
  );
  webviews.insert(new_window.0, new_window.1);

  event_loop.run(move |event, event_loop, control_flow| {
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
      Event::UserEvent(UserEvents::NewWindow()) => {
        let new_window = create_new_window(
          format!("Window {}", webviews.len() + 1),
          &event_loop,
          proxy.clone(),
          #[cfg(target_os = "macos")]
          process_pool.clone()
        );
        webviews.insert(new_window.0, new_window.1);
      }
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

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use dpi::LogicalSize;
use winit::{
  application::ApplicationHandler,
  event::WindowEvent,
  event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy},
  window::{Window, WindowId},
};
use wry::{http::Request, WebViewBuilder};

#[derive(Debug)]
enum UserEvent {
  TogglShadows,
}

struct App {
  window: Option<Window>,
  webview: Option<wry::WebView>,
  proxy: EventLoopProxy<UserEvent>,
  shadow: bool,
}

impl ApplicationHandler<UserEvent> for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    let window = event_loop
      .create_window(
        Window::default_attributes()
          .with_inner_size(LogicalSize::new(500u32, 500u32))
          .with_decorations(false),
      )
      .unwrap();

    const HTML: &str = r#"
  <html>

  <head>
      <style>
          html {
            font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
            width: 100vw;
            height: 100vh;
            background-color: #1f1f1f;
            border: 1px solid rgb(148, 231, 155);
          }

          * {
              padding: 0;
              margin: 0;
              box-sizing: border-box;
          }
      </style>
  </head>

  <body>
    <p>
      Click the window to toggle shadows.
    </p>

    <script>
      window.addEventListener('click', () => window.ipc.postMessage('toggleShadows'))
    </script>
  </body>

  </html>
"#;

    let proxy = self.proxy.clone();
    let handler = move |req: Request<String>| {
      if req.body().as_str() == "toggleShadows" {
        proxy.send_event(UserEvent::TogglShadows).unwrap();
      }
    };

    let webview = WebViewBuilder::new()
      .with_html(HTML)
      .with_ipc_handler(handler)
      .with_accept_first_mouse(true)
      .build_as_child(&window)
      .unwrap();

    self.window = Some(window);
    self.webview = Some(webview);
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _window_id: WindowId,
    event: WindowEvent,
  ) {
    match event {
      WindowEvent::CloseRequested => {
        let _ = self.webview.take();
        event_loop.exit();
      }
      _ => {}
    }
  }

  fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
    match event {
      UserEvent::TogglShadows => {
        self.shadow = !self.shadow;
        #[cfg(windows)]
        if let Some(window) = self.window.as_ref() {
          use winit::platform::windows::WindowExtWindows;
          window.set_undecorated_shadow(self.shadow);
        }
      }
    }
  }

  fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    #[cfg(any(
      target_os = "linux",
      target_os = "dragonfly",
      target_os = "freebsd",
      target_os = "netbsd",
      target_os = "openbsd",
    ))]
    while gtk4::glib::MainContext::default().iteration(false) {}
  }
}

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  {
    gtk4::init().unwrap();

    #[cfg(feature = "x11")]
    winit::platform::x11::register_xlib_error_hook(Box::new(|_display, error| {
      let error = error as *mut x11_dl::xlib::XErrorEvent;
      (unsafe { (*error).error_code }) == 170
    }));
  }

  let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
  let proxy = event_loop.create_proxy();
  let mut app = App {
    window: None,
    webview: None,
    proxy,
    shadow: true,
  };
  event_loop.run_app(&mut app).unwrap();
  Ok(())
}

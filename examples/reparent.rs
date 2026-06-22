// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "windows")]
use wry::WebViewExtWindows;
#[cfg(target_os = "macos")]
use {objc2_app_kit::NSWindow, wry::WebViewExtMacOS};

#[cfg(not(any(
  target_os = "windows",
  target_os = "macos",
  target_os = "ios",
  target_os = "android"
)))]
use wry::WebViewExtUnix;

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  ))]
  non_linux_main()?;

  #[cfg(not(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "ios",
    target_os = "android"
  )))]
  linux_main()?;

  Ok(())
}

/// Non-Linux implementation using winit.
#[cfg(any(
  target_os = "windows",
  target_os = "macos",
  target_os = "ios",
  target_os = "android"
))]
fn non_linux_main() -> wry::Result<()> {
  use dpi::{LogicalPosition, LogicalSize};
  use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{Key, NamedKey},
    window::{Window, WindowId},
  };
  use wry::{Rect, WebViewBuilder};

  struct App {
    window: Option<Window>,
    window2: Option<Window>,
    webview: Option<wry::WebView>,
    webview_in_window1: bool,
  }

  impl Default for App {
    fn default() -> Self {
      App {
        window: None,
        window2: None,
        webview: None,
        webview_in_window1: true,
      }
    }
  }

  impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
      let window = event_loop
        .create_window(Window::default_attributes().with_title("Window 1"))
        .unwrap();
      let window2 = event_loop
        .create_window(Window::default_attributes().with_title("Window 2"))
        .unwrap();

      let webview = WebViewBuilder::new()
        .with_url("https://tauri.app")
        .build_as_child(&window)
        .unwrap();

      self.window = Some(window);
      self.window2 = Some(window2);
      self.webview = Some(webview);
    }

    fn window_event(
      &mut self,
      event_loop: &ActiveEventLoop,
      _window_id: WindowId,
      event: WindowEvent,
    ) {
      match event {
        WindowEvent::Resized(size) => {
          if let (Some(window), Some(webview)) = (self.window.as_ref(), self.webview.as_ref()) {
            if self.webview_in_window1 {
              let size = size.to_logical::<u32>(window.scale_factor());
              webview
                .set_bounds(Rect {
                  position: LogicalPosition::new(0, 0).into(),
                  size: LogicalSize::new(size.width, size.height).into(),
                })
                .unwrap();
            }
          }
        }
        WindowEvent::CloseRequested => event_loop.exit(),
        WindowEvent::KeyboardInput {
          event:
            KeyEvent {
              logical_key: Key::Named(NamedKey::Space),
              state: ElementState::Pressed,
              ..
            },
          ..
        } => {
          let webview = match self.webview.as_ref() {
            Some(wv) => wv,
            None => return,
          };

          if self.webview_in_window1 {
            let window2 = self.window2.as_ref().unwrap();
            #[cfg(target_os = "macos")]
            {
              use winit::platform::macos::WindowExtMacOS;
              webview
                .reparent(window2.ns_window().cast::<NSWindow>().as_ptr())
                .unwrap();
            }
            #[cfg(target_os = "windows")]
            {
              use winit::platform::windows::WindowExtWindows;
              webview.reparent(window2.hwnd().0 as isize).unwrap();
            }
          } else {
            let window = self.window.as_ref().unwrap();
            #[cfg(target_os = "macos")]
            {
              use winit::platform::macos::WindowExtMacOS;
              webview
                .reparent(window.ns_window().cast::<NSWindow>().as_ptr())
                .unwrap();
            }
            #[cfg(target_os = "windows")]
            {
              use winit::platform::windows::WindowExtWindows;
              webview.reparent(window.hwnd().0 as isize).unwrap();
            }
          }
          self.webview_in_window1 = !self.webview_in_window1;
        }
        _ => {}
      }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {}
  }

  let event_loop = EventLoop::new().unwrap();
  let mut app = App::default();
  event_loop.run_app(&mut app).unwrap();
  Ok(())
}

/// Linux implementation using GTK4 directly (required for `reparent` which needs a GTK container).
#[cfg(not(any(
  target_os = "windows",
  target_os = "macos",
  target_os = "ios",
  target_os = "android"
)))]
fn linux_main() -> wry::Result<()> {
  use std::cell::Cell;
  use std::rc::Rc;

  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  gtk4::init().unwrap();

  let app = gtk4::Application::new(Some("com.example.reparent"), Default::default());
  app.connect_activate(|app| {
    let window1 = gtk4::ApplicationWindow::new(app);
    window1.set_title(Some("Window 1"));
    window1.set_default_size(800, 600);
    let vbox1 = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window1.set_child(Some(&vbox1));
    window1.present();

    let window2 = gtk4::ApplicationWindow::new(app);
    window2.set_title(Some("Window 2"));
    window2.set_default_size(800, 600);
    let vbox2 = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window2.set_child(Some(&vbox2));
    window2.present();

    let webview = wry::WebViewBuilder::new()
      .with_url("https://tauri.app")
      .build_gtk(&vbox1)
      .unwrap();

    let webview_rc = Rc::new(std::cell::RefCell::new(webview));
    let is_in_window1 = Rc::new(Cell::new(true));

    // Press 'x' to toggle the webview between the two windows
    let key_ctrl = gtk4::EventControllerKey::new();
    let webview_rc_clone = webview_rc.clone();
    let vbox1_clone = vbox1.clone();
    let vbox2_clone = vbox2.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
      if key == gtk4::gdk::Key::x {
        if is_in_window1.get() {
          webview_rc_clone.borrow().reparent(&vbox2_clone).unwrap();
          is_in_window1.set(false);
        } else {
          webview_rc_clone.borrow().reparent(&vbox1_clone).unwrap();
          is_in_window1.set(true);
        }
        return gtk4::glib::Propagation::Stop;
      }
      gtk4::glib::Propagation::Proceed
    });
    window1.add_controller(key_ctrl);
  });

  app.run();
  Ok(())
}

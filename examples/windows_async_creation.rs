// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Windows: create the WebView2 environment and controller without blocking
//! the event loop, then build the webview from them with
//! `WebViewBuilderExtWindows::with_environment` and `with_controller`.
//!
//! `WebViewBuilder::build` waits on the calling thread until WebView2 has
//! created both. Here the window keeps handling events while they are made:
//! the example counts the events it handled before the webview was ready.

#[cfg(target_os = "windows")]
fn main() -> wry::Result<()> {
  use std::{cell::RefCell, rc::Rc, time::Instant};

  use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::windows::WindowExtWindows,
    window::WindowBuilder,
  };
  use webview2_com::{
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    Microsoft::Web::WebView2::Win32::{
      CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2Controller, ICoreWebView2Environment,
    },
  };
  use windows::{core::PCWSTR, Win32::Foundation::HWND};
  use wry::{WebViewBuilder, WebViewBuilderExtWindows};

  enum UserEvent {
    ControllerReady,
  }

  let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
  let proxy = event_loop.create_proxy();
  let window = WindowBuilder::new()
    .with_title("Created without blocking")
    .build(&event_loop)
    .unwrap();
  let hwnd = HWND(window.hwnd() as _);

  // filled in by WebView2's completion handlers, which run on this thread
  // while the event loop below handles messages
  let made: Rc<RefCell<Option<(ICoreWebView2Environment, ICoreWebView2Controller)>>> =
    Rc::default();
  let started = Instant::now();

  let made_env = made.clone();
  let env_handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
    move |result, environment| {
      result?;
      let environment = environment.expect("an environment");
      let env = environment.clone();
      let controller_handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
        move |result, controller| {
          result?;
          made_env.replace(Some((env, controller.expect("a controller"))));
          let _ = proxy.send_event(UserEvent::ControllerReady);
          Ok(())
        },
      ));
      // the controller can be created on any window: wry moves it into the
      // webview's own container when the webview is built
      unsafe { environment.CreateCoreWebView2Controller(hwnd, &controller_handler) }
    },
  ));
  unsafe {
    CreateCoreWebView2EnvironmentWithOptions(PCWSTR::null(), PCWSTR::null(), None, &env_handler)
      .expect("WebView2 is installed");
  }

  let mut webview = None;
  let mut events_while_waiting = 0u64;
  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;
    match event {
      Event::UserEvent(UserEvent::ControllerReady) => {
        let (environment, controller) = made.borrow_mut().take().unwrap();
        webview = Some(
          WebViewBuilder::new()
            .with_environment(environment)
            .with_controller(controller)
            .with_html("<h1>Created without blocking the event loop</h1>")
            .build(&window)
            .unwrap(),
        );
        println!(
          "webview ready after {} ms; the event loop handled {events_while_waiting} events meanwhile",
          started.elapsed().as_millis()
        );
      }
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      _ if webview.is_none() => events_while_waiting += 1,
      _ => {}
    }
  });
}

#[cfg(not(target_os = "windows"))]
fn main() {
  println!("This example is for Windows (WebView2) only.");
}

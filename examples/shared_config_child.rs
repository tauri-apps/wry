// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Regression check for a macOS child webview built on its opener's configuration.
//!
//! `window.open` is answered with [`NewWindowResponse::Create`] and a webview built on
//! `opener.target_configuration`. WebKit's configuration copy shares the opener's
//! `WKUserContentController`, so the child must neither add its scripts to it nor remove the
//! opener's `ipc` handler from it when it is dropped.
//!
//! The check records the opener's user script count before the child opens, after it opens and
//! after it is dropped, then has the opener post through `window.ipc.postMessage`. It passes
//! (exit code 0) when the count never changes and the message arrives; it fails with exit code 1
//! otherwise, and 2 when the child could not be opened at all.
//!
//! The windows are never shown and the app is never activated, unless
//! `WRY_SHARED_CONFIG_VISIBLE=1` is set.
//!
//! ```sh
//! cargo run --example shared_config_child
//! ```

#[cfg(target_os = "macos")]
fn main() {
  macos::main();
}

#[cfg(not(target_os = "macos"))]
fn main() {
  println!("This example only runs on macOS.");
}

#[cfg(target_os = "macos")]
mod macos {
  use std::{cell::RefCell, rc::Rc, thread, time::Duration};

  use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::macos::{ActivationPolicy, EventLoopExtMacOS},
    window::{Window, WindowBuilder},
  };
  use wry::{
    http::Request, NewWindowResponse, WebView, WebViewBuilder, WebViewBuilderExtMacos,
    WebViewExtMacOS,
  };

  enum UserEvent {
    MainIpc(String),
    ChildCreated,
    CloseChild,
    Timeout,
  }

  const MAIN_HTML: &str = r#"<!doctype html><html><body>main
<script>window.addEventListener('load', () => window.ipc.postMessage('ready'));</script>
</body></html>"#;

  fn after(proxy: &EventLoopProxy<UserEvent>, delay: Duration, event: UserEvent) {
    let proxy = proxy.clone();
    thread::spawn(move || {
      thread::sleep(delay);
      let _ = proxy.send_event(event);
    });
  }

  pub fn main() {
    let visible = std::env::var("WRY_SHARED_CONFIG_VISIBLE").is_ok_and(|value| value == "1");

    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    event_loop.set_activation_policy(if visible {
      ActivationPolicy::Accessory
    } else {
      ActivationPolicy::Prohibited
    });
    event_loop.set_activate_ignoring_other_apps(false);
    let proxy = event_loop.create_proxy();

    let main_window = WindowBuilder::new()
      .with_title("shared_config_child: main")
      .with_visible(visible)
      .with_focused(false)
      .build(&event_loop)
      .unwrap();
    let child_window: Rc<RefCell<Option<Window>>> = Rc::new(RefCell::new(Some(
      WindowBuilder::new()
        .with_title("shared_config_child: child")
        .with_visible(visible)
        .with_focused(false)
        .build(&event_loop)
        .unwrap(),
    )));
    let child_webview: Rc<RefCell<Option<WebView>>> = Rc::new(RefCell::new(None));

    let ipc_proxy = proxy.clone();
    let new_window_proxy = proxy.clone();
    let handler_child_window = child_window.clone();
    let handler_child_webview = child_webview.clone();
    let main_webview = WebViewBuilder::new()
      .with_html(MAIN_HTML)
      .with_ipc_handler(move |request: Request<String>| {
        let _ = ipc_proxy.send_event(UserEvent::MainIpc(request.body().clone()));
      })
      .with_new_window_req_handler(move |url, features| {
        println!("new window requested: {url}");
        let window = handler_child_window.borrow();
        let Some(window) = window.as_ref() else {
          return NewWindowResponse::Deny;
        };
        // Built the way an app answers `window.open` with its own webview: its own IPC handler
        // and init scripts, on the configuration WebKit hands over.
        let webview = WebViewBuilder::new()
          .with_webview_configuration(features.opener.target_configuration)
          .with_initialization_script("window.__shared_config_child = true;")
          .with_ipc_handler(|request: Request<String>| {
            println!("child ipc: {}", request.body());
          })
          .build(window)
          .unwrap();
        let wk_webview = objc2::rc::Retained::into_super(webview.webview());
        *handler_child_webview.borrow_mut() = Some(webview);
        let _ = new_window_proxy.send_event(UserEvent::ChildCreated);
        NewWindowResponse::Create {
          webview: wk_webview,
        }
      })
      .build(&main_window)
      .unwrap();

    // Let `window.open` from script through without a user gesture.
    unsafe {
      main_webview
        .webview()
        .configuration()
        .preferences()
        .setJavaScriptCanOpenWindowsAutomatically(true);
    }

    let user_scripts = |webview: &WebView| unsafe { webview.manager().userScripts().count() };
    let mut before = None;
    let mut after_open = None;
    let mut after_close = None;
    let mut closed = false;

    after(&proxy, Duration::from_secs(10), UserEvent::Timeout);

    event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;
      let Event::UserEvent(event) = event else {
        return;
      };

      let finish = |code: i32, reason: &str, counts: [Option<usize>; 3]| -> ! {
        let [before, after_open, after_close] = counts;
        println!(
          "user scripts before={before:?} after_open={after_open:?} after_close={after_close:?}"
        );
        println!("{}: {reason}", if code == 0 { "PASS" } else { "FAIL" });
        std::process::exit(code);
      };

      match event {
        UserEvent::MainIpc(body) if body == "ready" && before.is_none() => {
          before = Some(user_scripts(&main_webview));
          println!("main ipc: ready");
          main_webview
            .evaluate_script("window.open('about:blank#popout-1')")
            .unwrap();
        }
        UserEvent::ChildCreated => {
          after_open = Some(user_scripts(&main_webview));
          // Give the child time to load before it is dropped.
          after(&proxy, Duration::from_millis(500), UserEvent::CloseChild);
        }
        UserEvent::CloseChild => {
          child_webview.borrow_mut().take();
          child_window.borrow_mut().take();
          closed = true;
          after_close = Some(user_scripts(&main_webview));
          main_webview
            .evaluate_script(
              "try { window.ipc.postMessage('after-close') } catch (e) { console.error(e) }",
            )
            .unwrap();
        }
        UserEvent::MainIpc(body) if body == "after-close" => {
          println!("main ipc: after-close");
          let counts = [before, after_open, after_close];
          if after_open == before && after_close == before {
            finish(
              0,
              "main's ipc delivers after the child closed and its user scripts are unchanged",
              counts,
            );
          } else {
            finish(
              1,
              "main's user scripts changed while the child was open or after it closed",
              counts,
            );
          }
        }
        UserEvent::MainIpc(body) => println!("main ipc: {body}"),
        UserEvent::Timeout if closed => finish(
          1,
          "main's ipc message after the child closed never arrived",
          [before, after_open, after_close],
        ),
        UserEvent::Timeout => {
          println!("FAIL: the child webview was never created (window.open did not reach Create)");
          std::process::exit(2);
        }
      }
    });
  }
}

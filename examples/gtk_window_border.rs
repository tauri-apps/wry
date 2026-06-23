// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! GTK4-native equivalent of the `window_border` example.
//!
//! Demonstrates toggling GTK4 window decorations (title bar and WM chrome) via
//! wry IPC.  When decorations are removed the window becomes frameless; a CSS
//! border drawn by the webview serves as the only visible window edge.  When
//! decorations are restored the WM title bar returns and the CSS border hides.
//!
//! Click flow: JS click → `window.ipc.postMessage` → Rust IPC handler →
//! `gtk4::Window::set_decorated` + `WebView::evaluate_script` → CSS update.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_window_border
//! ```
//!
//! See LINUX.md for troubleshooting tips.

fn main() -> wry::Result<()> {
  #[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  ))]
  return linux_main();

  #[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
  )))]
  {
    eprintln!("gtk_window_border is a Linux/BSD-only example.");
    Ok(())
  }
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
fn linux_main() -> wry::Result<()> {
  use std::cell::RefCell;
  use std::rc::Rc;

  use gtk4::prelude::*;
  use wry::WebViewBuilderExtUnix;

  // When decorated the WM provides the window frame so corner markers are
  // hidden.  When undecorated, four L-shaped corner brackets appear as the
  // only visible window boundary.
  const HTML: &str = r#"
<html>
<head>
  <style>
    *, html, body { margin: 0; padding: 0; box-sizing: border-box; }
    html, body {
      width: 100%; height: 100%;
      background: rgba(20, 20, 20, 0.92);
      font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
      color: #94e79b;
      overflow: hidden;
    }
    body { padding: 1.2rem; }
    p { margin-bottom: 0.4rem; }
    #hint { opacity: 0.6; font-size: 0.85em; }

    /* L-shaped corner brackets — hidden until undecorated. */
    .corner {
      position: fixed;
      width: 20px;
      height: 20px;
      border: 2px solid rgb(148, 231, 155);
      opacity: 0;
      transition: opacity 0.15s ease;
      pointer-events: none;
    }
    .corner.tl { top: 0; left: 0;  border-right: none; border-bottom: none; }
    .corner.tr { top: 0; right: 0; border-left: none;  border-bottom: none; }
    .corner.bl { bottom: 0; left: 0;  border-right: none; border-top: none; }
    .corner.br { bottom: 0; right: 0; border-left: none;  border-top: none; }

    html.undecorated .corner { opacity: 1; }
  </style>
</head>
<body>
  <div class="corner tl"></div>
  <div class="corner tr"></div>
  <div class="corner bl"></div>
  <div class="corner br"></div>

  <p id="state">Decorations: ON</p>
  <p id="hint">Click anywhere to toggle the window title bar.</p>
  <script>
    window.addEventListener('click', () => window.ipc.postMessage('toggleDecorations'));
    window.setDecorated = function(on) {
      document.documentElement.classList.toggle('undecorated', !on);
      document.getElementById('state').textContent = 'Decorations: ' + (on ? 'ON' : 'OFF');
    };
  </script>
</body>
</html>
"#;

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Window Border (GTK4)"));
    window.set_default_size(500, 300);

    // Transparent surface with square corners so the CSS corner brackets
    // align correctly when the WM title bar is removed.  GTK4 CSD windows
    // have rounded top corners by default; border-radius: 0 overrides that.
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(
      "window, .background { background-color: transparent; border-radius: 0; }",
    );
    if let Some(display) = gtk4::gdk::Display::default() {
      gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
      );
    }

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    // Share the webview with the IPC handler so it can call evaluate_script.
    let webview_ref: Rc<RefCell<Option<wry::WebView>>> = Rc::new(RefCell::new(None));
    let webview_for_ipc = Rc::clone(&webview_ref);

    // IPC fires on the GTK main thread — safe to call GTK methods directly.
    let window_clone = window.clone();
    let webview = wry::WebViewBuilder::new()
      .with_transparent(true)
      .with_html(HTML)
      .with_ipc_handler(move |req: wry::http::Request<String>| {
        if req.body().as_str() == "toggleDecorations" {
          let decorated = !window_clone.is_decorated();
          window_clone.set_decorated(decorated);
          if let Some(wv) = webview_for_ipc.borrow().as_ref() {
            let _ = wv.evaluate_script(&format!("window.setDecorated({})", decorated));
          }
        }
      })
      .build_gtk(&vbox)
      .unwrap();

    *webview_ref.borrow_mut() = Some(webview);

    window.connect_close_request(move |_| {
      webview_ref.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

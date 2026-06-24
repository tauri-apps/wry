// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Exercises Linux-specific WebView features added in the gtk4-webkit6 backend:
//!
//! - `with_hardware_acceleration_policy(Never)` — force software rendering
//! - `with_theme(Dark)` — start in dark mode; press **T** to toggle Dark/Light
//! - `with_on_web_content_process_terminate_handler` — crash handler; press **C** to trigger
//! - `with_data_directory(path)` — isolated `NetworkSession` data directory
//! - `data_directory()` accessor — read back the active data path at runtime
//!
//! Expected stdout on startup:
//!
//! ```text
//! [hardware_acceleration] policy = Never (software rendering)
//! [theme]                 started in Dark mode
//! [data_directory]        requested: /tmp/wry-gtk-features-demo
//! [data_directory]        accessor  : /tmp/wry-gtk-features-demo  (or None if no page loaded yet)
//! [keys]                  T = toggle theme | C = crash web process | Q/Esc = quit
//! ```
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_linux_features
//! ```
//!
//! See `LINUX.md` for troubleshooting (DMA-BUF, Wayland socket, etc.).

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
    eprintln!("gtk_linux_features is a Linux/BSD-only example.");
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
  use std::cell::{Cell, RefCell};
  use std::rc::Rc;

  use gtk4::prelude::*;
  use webkit6::{prelude::WebViewExt as WebKitViewExt, HardwareAccelerationPolicy};
  use wry::{Theme, WebViewBuilderExtUnix, WebViewExtUnix};

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Linux Features (GTK4 / webkit6)"));
    window.set_default_size(900, 650);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    // --- 3a: with_data_directory ---
    // Use a fixed temp path so we can verify the directory on disk.
    let data_dir = std::env::temp_dir().join("wry-gtk-features-demo");

    let webview = wry::WebViewBuilder::new()
      .with_html(HTML)
      // --- 2b: with_hardware_acceleration_policy ---
      // HardwareAccelerationPolicy::Never forces WebKit to use software
      // rendering (llvmpipe / softpipe). No GPU / DMA-BUF support required.
      // Equivalent to WEBKIT_DISABLE_DMABUF_RENDERER=1 but per-webview.
      .with_hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
      // --- 2d: with_theme ---
      // Sets the GTK display setting gtk-application-prefer-dark-theme so
      // WebKitGTK resolves prefers-color-scheme correctly on first render.
      // webkit6 0.6.x has no runtime set_preferred_color_scheme(); runtime
      // toggling is done via evaluate_script + data-theme attribute (see key T).
      .with_theme(Theme::Dark)
      // --- 3a: with_data_directory ---
      // Creates an isolated NetworkSession whose cache, IndexedDB, cookies, etc.
      // live under this path instead of the default system WebKit directory.
      .with_data_directory(data_dir.clone())
      // --- 2a: with_on_web_content_process_terminate_handler ---
      // Called when the WebKit web process crashes (about:crash) or exceeds
      // its memory limit. Connect webkit6's web-process-terminated signal
      // directly via WebViewExtUnix::webview() if you need the reason enum.
      .with_on_web_content_process_terminate_handler(|| {
        println!("[web_process_terminated] web process crashed or was killed — reloading");
      })
      .build_gtk(&vbox)
      .unwrap();

    // Connect a second terminate handler directly on the underlying webkit6 view.
    // wry's with_on_web_content_process_terminate_handler drops the WebView reference
    // so it can't reload. This signal handler has the view in scope and can.
    {
      use webkit6::prelude::WebViewExt as WK;
      webview.webview().connect_web_process_terminated(|view, reason| {
        println!("[web_process_terminated] reason: {reason:?}");
        view.load_html(HTML, None::<&str>);
      });
    }

    // --- 3b: data_directory() accessor ---
    // Returns None until a NetworkSession with a data directory is created,
    // which happens during webview construction when with_data_directory is set.
    println!(
      "[hardware_acceleration] policy = Never (software rendering)"
    );
    println!("[theme]                 started in Dark mode");
    println!("[data_directory]        requested: {}", data_dir.display());
    match webview.data_directory() {
      Some(p) => println!("[data_directory]        accessor  : {}", p.display()),
      None => println!("[data_directory]        accessor  : None (ephemeral session)"),
    }
    println!("[keys]                  T = toggle theme | C = crash web process | Q/Esc = quit");

    let webview = Rc::new(RefCell::new(Some(webview)));
    let is_dark = Rc::new(Cell::new(true));

    let key_ctrl = gtk4::EventControllerKey::new();
    let webview_clone = webview.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
      match key {
        // T — toggle dark/light by setting a data-theme attribute on the page.
        // webkit6 0.6.x has no runtime set_preferred_color_scheme() binding,
        // so we drive the HTML page's own CSS variables instead (see HTML const).
        gtk4::gdk::Key::t | gtk4::gdk::Key::T => {
          let dark = !is_dark.get();
          is_dark.set(dark);
          if let Some(wv) = webview_clone.borrow().as_ref() {
            let attr = if dark { "dark" } else { "light" };
            let _ = wv.evaluate_script(&format!(
              "document.documentElement.setAttribute('data-theme','{attr}')"
            ));
            println!(
              "[theme]                 switched to {} (data-theme={attr})",
              if dark { "Dark" } else { "Light" }
            );
          }
          gtk4::glib::Propagation::Stop
        }

        // C — terminate the web process via the webkit6 API.
        // terminate_web_process() kills the renderer immediately, triggering
        // the with_on_web_content_process_terminate_handler closure above.
        // The webview goes blank afterwards; restart the app to reload.
        gtk4::gdk::Key::c | gtk4::gdk::Key::C => {
          println!("[crash]                 calling terminate_web_process() — handler should fire");
          if let Some(wv) = webview_clone.borrow().as_ref() {
            wv.webview().terminate_web_process();
          }
          gtk4::glib::Propagation::Stop
        }

        _ => gtk4::glib::Propagation::Proceed,
      }
    });
    window.add_controller(key_ctrl);

    window.connect_close_request(move |_| {
      webview.borrow_mut().take();
      gtk4::glib::Propagation::Proceed
    });
  });

  app.run();
  Ok(())
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd",
))]
const HTML: &str = r#"
<html>
<head>
  <style>
    *, html, body { margin: 0; padding: 0; box-sizing: border-box; }
    html, body { width: 100%; height: 100%; }
    body {
      background: rgba(20, 20, 20, 0.92);
      color: #94e79b;
      font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
      padding: 1.2rem;
      transition: background 0.25s, color 0.25s;
    }
    html[data-theme="light"] body {
      background: #f0f0f0;
      color: #1a4a1f;
    }
    p { margin-bottom: 0.35rem; }
    .dim { opacity: 0.6; font-size: 0.85em; margin-bottom: 0.9rem; margin-left: 1.4rem; }
    #status { margin-top: 1.2rem; opacity: 0.55; font-size: 0.82em; }
  </style>
</head>
<body>
  <p>Linux Features Demo</p>
  <br>
  <p><b>T</b> &mdash; Toggle theme</p>
  <p class="dim">Switches the page between Dark and Light by setting a <code>data-theme</code>
  attribute on the root element. Current: <span id="theme-name">Dark</span>.</p>

  <p><b>C</b> &mdash; Crash the web process</p>
  <p class="dim">Calls <code>terminate_web_process()</code> on the underlying webkit6 view.
  The <code>with_on_web_content_process_terminate_handler</code> closure fires,
  then the page reloads automatically.</p>

  <p id="status">Rendering: software (HardwareAccelerationPolicy::Never) &nbsp;|&nbsp; Data dir: see terminal</p>

  <script>
    // Reflect the data-theme attribute set by Rust evaluate_script calls.
    const obs = new MutationObserver(() => {
      const t = document.documentElement.getAttribute('data-theme') || 'Dark';
      document.getElementById('theme-name').textContent =
        t.charAt(0).toUpperCase() + t.slice(1);
    });
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  </script>
</body>
</html>
"#;

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Permission handler example for GTK4 / webkit6.
//!
//! Toggle Allow / Default / Deny for each permission kind.
//! Choices are persisted in session cookies so they survive page reloads.
//! All cookies are wiped on exit.
//!
//! Run with:
//!
//! ```bash
//! cargo run --example gtk_permission_handler
//! ```

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
    eprintln!("gtk_permission_handler is a Linux/BSD-only example.");
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
  use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, Mutex},
  };

  use gtk4::prelude::*;
  use webkit6::prelude::WebViewExt as Webkit6WebViewExt;
  use wry::{PermissionResponse, WebViewBuilderExtUnix, WebViewExtUnix};

  let app = gtk4::Application::new(None::<&str>, Default::default());

  app.connect_activate(|app| {
    let window = gtk4::ApplicationWindow::new(app);
    window.set_title(Some("Permission Handler — GTK4 / webkit6"));
    window.set_default_size(700, 540);

    let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    window.set_child(Some(&vbox));
    window.present();

    // Shared permission state: "geolocation" -> Allow/Deny/Default
    let state: Arc<Mutex<HashMap<String, PermissionResponse>>> =
      Arc::new(Mutex::new(HashMap::new()));

    let state_ipc = state.clone();
    let state_perm = state.clone();

    let webview = wry::WebViewBuilder::new()
      // Serve via a custom protocol so the page gets a real secure origin
      // (wry://localhost). with_html() gives a null origin, which causes
      // webkit6 to reject Geolocation / Camera / etc. before the permission
      // handler is ever invoked.
      .with_custom_protocol("wry".into(), |_id, _req| {
        wry::http::Response::builder()
          .header("Content-Type", "text/html; charset=utf-8")
          .body(HTML.as_bytes().to_vec())
          .unwrap()
          .map(Into::into)
      })
      .with_url("wry://localhost")
      // JS sends "kind:value" whenever a radio button changes.
      // On page load it replays the cookie state so Rust stays in sync.
      .with_ipc_handler(move |req| {
        let body = req.body().to_string();
        if let Some((kind, value)) = body.split_once(':') {
          let response = match value {
            "allow" => PermissionResponse::Allow,
            "deny" => PermissionResponse::Deny,
            _ => PermissionResponse::Default,
          };
          state_ipc
            .lock()
            .unwrap()
            .insert(kind.to_string(), response);
          println!("[toggle]      {kind:24} = {value}");
        }
      })
      .with_permission_handler(move |kind| {
        let key = kind.to_string();
        let response = state_perm
          .lock()
          .unwrap()
          .get(&key)
          .copied()
          .unwrap_or(PermissionResponse::Default);
        println!("[permission]  {kind:24} -> {response}");
        response
      })
      .build_gtk(&vbox)
      .unwrap();

    // Enable APIs that webkit6 disables by default.
    // Must be set after build; these are not exposed through wry's builder.
    if let Some(settings) = Webkit6WebViewExt::settings(&webview.webview()) {
      settings.set_enable_media_stream(true); // Camera / Microphone
      settings.set_enable_encrypted_media(true); // MediaKeySystem (EME/DRM)
    }

    let webview = RefCell::new(Some(webview));
    window.connect_close_request(move |_| {
      // Wipe all cookies/browsing data before the webview is dropped.
      if let Some(wv) = webview.borrow().as_ref() {
        let _ = wv.clear_all_browsing_data();
        println!("[exit]        browsing data cleared");
      }
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
const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Permission Handler</title>
<style>
  body   { font-family: system-ui, sans-serif; max-width: 660px;
           margin: 2rem auto; padding: 0 1rem; color: #222; }
  h2     { margin-bottom: 0.25rem; }
  p.sub  { color: #555; font-size: 0.9rem; margin-bottom: 1.5rem; }

  table  { width: 100%; border-collapse: collapse; margin-bottom: 1.5rem; }
  th     { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 2px solid #ccc;
           font-size: 0.85rem; color: #555; }
  td     { padding: 0.45rem 0.6rem; border-bottom: 1px solid #eee; font-size: 0.9rem; }
  td:not(:first-child) { text-align: center; }

  label  { cursor: pointer; }
  input[type=radio] { accent-color: #0070f3; cursor: pointer; }

  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 1.5rem; }
  button { padding: 0.35rem 0.9rem; border: 1px solid #ccc; border-radius: 4px;
           background: #f5f5f5; cursor: pointer; font-size: 0.85rem; }
  button:hover { background: #e8e8e8; }

  #log   { background: #f9f9f9; border: 1px solid #ddd; border-radius: 4px;
           padding: 0.6rem 0.8rem; font-family: monospace; font-size: 0.8rem;
           min-height: 4rem; max-height: 160px; overflow-y: auto;
           white-space: pre-wrap; color: #333; }
</style>
</head>
<body>
<h2>Permission Handler</h2>
<p class="sub">
  Toggle each permission below. Choices are saved in cookies for this session
  and wiped automatically when the window closes.<br>
  <b>Note:</b> <i>Default</i> on Linux = Deny (webkit6 has no native permission prompt).
  Granting permission does not guarantee the feature works — system services
  (geoclue2, PipeWire, etc.) must also be running.
</p>

<table>
  <thead>
    <tr>
      <th>Permission</th>
      <th>Allow</th>
      <th>Default</th>
      <th>Deny</th>
    </tr>
  </thead>
  <tbody id="perm-rows"></tbody>
</table>

<p style="font-size:0.85rem;color:#555;margin-bottom:0.6rem">
  Trigger a request to test the current setting:
</p>
<div class="actions" id="triggers"></div>

<div id="log">Waiting for permission events…</div>

<script>
// Permissions routed through wry's permission handler on webkit6.
// "key" matches PermissionKind::to_string() (Display impl in permissions.rs).
// "req" = system service required for the feature to actually work after Allow.
const PERMS = [
  { key: 'geolocation',            label: 'Geolocation',           def: 'default', req: 'geoclue2'              },
  { key: 'notifications',          label: 'Notifications',         def: 'allow',   req: 'notification daemon'   },
  { key: 'camera',                 label: 'Camera',                def: 'allow',   req: 'PipeWire + v4l2'       },
  { key: 'microphone',             label: 'Microphone',            def: 'allow',   req: 'PipeWire / PulseAudio' },
  { key: 'clipboard-read',         label: 'Clipboard Read',        def: 'allow',   req: null                    },
  { key: 'pointer-lock',           label: 'Pointer Lock',          def: 'allow',   req: null                    },
  { key: 'media-key-system-access',label: 'MediaKeySystem (EME)',  def: 'deny',    req: 'Widevine / GStreamer'   },
];

// --- Cookie helpers ---
function setCookie(name, value) {
  // Session cookie (no Max-Age) so it dies with the browser session.
  // Rust also calls clear_all_browsing_data() on exit for belt-and-suspenders.
  document.cookie = name + '=' + value + '; path=/; SameSite=Strict';
}
function getCookie(name) {
  const m = document.cookie.match('(?:^|; )' + name + '=([^;]*)');
  return m ? m[1] : null;
}

// --- Build table rows ---
const tbody = document.getElementById('perm-rows');
const triggers = document.getElementById('triggers');
const logEl = document.getElementById('log');

function log(msg) {
  logEl.textContent += '\n' + msg;
  logEl.scrollTop = logEl.scrollHeight;
}

PERMS.forEach(({ key, label, def, req }) => {
  const stored = getCookie('perm_' + key) || def;

  // Tell Rust the initial value from cookies.
  window.ipc.postMessage(key + ':' + stored);

  // Table row
  const tr = document.createElement('tr');
  const labelCell = req
    ? label + ' <span style="color:#888;font-size:0.78rem">(' + req + ')</span>'
    : label;
  tr.innerHTML = '<td>' + labelCell + '</td>' +
    ['allow', 'default', 'deny'].map(v =>
      '<td><label><input type="radio" name="' + key + '" value="' + v + '"' +
      (stored === v ? ' checked' : '') + '> ' + v + '</label></td>'
    ).join('');
  tbody.appendChild(tr);

  // Wire up change handler
  tr.querySelectorAll('input[type=radio]').forEach(input => {
    input.addEventListener('change', () => {
      setCookie('perm_' + key, input.value);
      window.ipc.postMessage(key + ':' + input.value);
      log('[toggle] ' + label + ' = ' + input.value);
    });
  });

  // Trigger button
  const btn = document.createElement('button');
  btn.textContent = label;
  btn.onclick = () => trigger(key, label);
  triggers.appendChild(btn);
});

// --- Trigger functions ---
function trigger(key, label) {
  log('[request] ' + label + '...');
  const ok  = v  => log('  granted: ' + (typeof v === 'object' && v ? v.constructor.name : String(v)));
  const err = e  => log('  denied/error: ' + e);

  switch (key) {
    case 'geolocation':
      navigator.geolocation.getCurrentPosition(
        p => ok('lat=' + p.coords.latitude.toFixed(4)),
        e => err(e.message)
      );
      break;

    case 'notifications':
      Notification.requestPermission().then(ok).catch(err);
      break;

    case 'camera':
      navigator.mediaDevices.getUserMedia({ video: true })
        .then(s => { ok('MediaStream'); s.getTracks().forEach(t => t.stop()); })
        .catch(err);
      break;

    case 'microphone':
      navigator.mediaDevices.getUserMedia({ audio: true })
        .then(s => { ok('MediaStream'); s.getTracks().forEach(t => t.stop()); })
        .catch(err);
      break;

    case 'clipboard-read':
      navigator.clipboard.readText().then(t => ok('"' + t.slice(0, 40) + '"')).catch(err);
      break;

    case 'pointer-lock':
      document.body.requestPointerLock();
      break;

    case 'media-key-system-access':
      navigator.requestMediaKeySystemAccess('com.example.drm', [
        { initDataTypes: ['cenc'], videoCapabilities: [{ contentType: 'video/mp4' }] }
      ]).then(a => ok(a.keySystem)).catch(err);
      break;
  }
}

document.addEventListener('pointerlockchange', () => {
  log(document.pointerLockElement
    ? '  pointer locked (press Esc to release)'
    : '  pointer released');
});
document.addEventListener('pointerlockerror', () => log('  pointer lock error'));
</script>
</body>
</html>
"#;

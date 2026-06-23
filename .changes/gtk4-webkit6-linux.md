---
"wry": minor
---

On Linux, migrate from gtk3/webkit2gtk to gtk4/webkit6.

This resolves unsoundness advisory GHSA-wrw7-89jp-8q8g (glib < 0.2). The Linux
backend now requires gtk4 and webkit6 (WebKitGTK 6.x). Public API on Linux has
changed: `WebViewBuilder` now accepts a `gtk4::Widget` container, and `webkit2gtk`
types are replaced with their `webkit6` equivalents. GTK-specific examples have
been updated from tao+gtk3 to winit+gtk4.

**Breaking changes:**

- Linux dependency packages updated: `libwebkit2gtk-4.1-dev` → `libwebkitgtk-6.0-dev` + `libgtk-4-dev`
  - Arch/Manjaro: `webkit2gtk-4.1` → `webkitgtk-6.0`
  - Debian/Ubuntu: `libwebkit2gtk-4.1-dev` → `libwebkitgtk-6.0-dev`
  - Fedora: `gtk3-devel webkit2gtk4.1-devel` → `gtk4-devel webkitgtk6.0-devel`
  - Nix/NixOS: `webkitgtk_4_1` → `webkitgtk_6_0`
- GTK integration changed from gtk3 (`gtk::init`, `gtk::main_iteration_do`) to gtk4
  (`gtk4::init`, `gtk4::glib::MainContext::default()`)
- `WebViewBuilderExtUnix::new_gtk` renamed to `WebViewBuilderExtUnix::build_gtk`

**Bug fixes:**

- Fixed webview not rendering when built into a `gtk4::Box` container via `build_gtk`.
  The `WebView` widget was not marked as expanding (`hexpand`/`vexpand`), so it collapsed
  to zero size — WebKit loaded pages correctly but rendered into an invisible area.
  This affected all `build_gtk` usage with `gtk4::Box`, including `reparent` and any
  Wayland-native application layout.

**New GTK4-native (Wayland) examples:**

A full set of `gtk_*` examples has been added as Wayland-native counterparts to
every winit example.  These use `build_gtk` with a GTK4 `Application` and work on
both Wayland and X11.

- `gtk_simple` — minimal webview in a GTK4 `ApplicationWindow`
- `gtk_multiwebview` — 2×2 grid of webviews using nested `Box` containers
- `gtk_multiwindow` — multiple windows opened/closed/renamed via IPC
- `gtk_cookies` — cookie set, list, and delete
- `gtk_custom_protocol` — custom `wry://` protocol handler
- `gtk_async_custom_protocol` — asynchronous custom protocol handler
- `gtk_streaming` — HTTP Range / video streaming via `stream://` protocol
- `gtk_transparent` — transparent webview with CSS window background
- `gtk_custom_titlebar` — custom header bar using `WindowHandle` + GTK4 buttons
- `gtk_window_border` — undecorated window with CSS border; click toggles decorations
- `gtk_permission_handler` — browser permission API with custom handler
- `gtk_opengl` — GTK4 `GLArea` + transparent webview overlay

See `LINUX.md` for run commands and troubleshooting.

**CI / tooling:**

- All CI workflows updated to install `libwebkitgtk-6.0-dev` and `libgtk-4-dev`
- Bench workflow sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux to avoid DMA-BUF
  GPU driver issues in the `xvfb` headless environment

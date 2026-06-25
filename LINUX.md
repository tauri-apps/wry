# Linux Development Notes

## Table of Contents

- [Examples](#examples)
  - [Wayland — GTK4 / webkit6](#wayland--gtk4--webkit6)
  - [X11 — winit](#x11--winit)
- [Feature Notes](#feature-notes)
  - [Wayland Native Embedding](#wayland-native-embedding)
  - [Streaming](#streaming)
  - [Permission Handler](#permission-handler)
  - [Theme / Dark Mode](#theme--dark-mode)
  - [Focus Handler](#focus-handler)
  - [Keyboard Handler](#keyboard-handler)
  - [Clipboard](#clipboard)
  - [Drag Source](#drag-source-outgoing-drags)
  - [Pointer Motion / Enter / Leave](#pointer-motion--enter--leave)
  - [Scroll Interception](#scroll-interception)
  - [Cursor Control](#cursor-control)
  - [Primary Clipboard (X11 Selection)](#primary-clipboard-x11-selection)
  - [Monitor Change Notifications](#monitor-change-notifications)
  - [Web Process Crash Handler](#web-process-crash-handler)
  - [New Window Requests](#new-window-requests)
  - [Isolated Data Directory](#isolated-data-directory)
  - [Cookie Accept Policy](#cookie-accept-policy)
  - [Data Directory Accessor](#data-directory-accessor)
- [Known Issues](#known-issues)
  - [GLIBC_PRIVATE symbol error](#glibc_private-symbol-error)
  - [NeedDebuggerBreak trap in stderr](#needdebuggerbeak-trap-in-stderr)
  - [DMA-BUF rendering issues](#dma-buf-rendering-issues)

---

## Examples

WRY has two families of examples on Linux:

| Family | Backend | X11 | Wayland |
|--------|---------|-----|---------|
| `gtk_*` | GTK4 / webkit6 | ✓ | ✓ |
| All others | winit + `--features x11` | ✓ | ✗ |
| Raw handle | `--features wayland` | ✗ | ✓ (GTK-owned surface required) |

### Wayland — GTK4 / webkit6

| Example | Winit equivalent | Description |
|---|---|---|
| `gtk_simple` | `simple` | Basic webview |
| `gtk_multiwebview` | `multiwebview` | 2×2 grid of webviews |
| `gtk_multiwindow` | `multiwindow` | Multiple windows via IPC |
| `gtk_cookies` | `cookies` | Cookie set/list/delete |
| `gtk_custom_protocol` | `custom_protocol` | Custom protocol handler |
| `gtk_async_custom_protocol` | `async_custom_protocol` | Async protocol handler |
| `gtk_streaming` | `streaming` | HTTP Range / video streaming |
| `gtk_transparent` | `transparent` | Transparent webview |
| `gtk_custom_titlebar` | `custom_titlebar` | Custom window titlebar |
| `gtk_window_border` | `window_border` | Undecorated window with border |
| `gtk_permission_handler` | `permission_handler` | Browser permission API |
| `gtk_opengl` | *(linux only)* | OpenGL + WebView overlay |
| `gtk_linux_features` | *(linux only)* | Hardware accel policy, theme, crash handler, data directory |
| `reparent` | `reparent` | Move webview between containers |

```bash
cargo run --example gtk_simple
cargo run --example gtk_multiwebview
cargo run --example gtk_multiwindow
cargo run --example gtk_opengl
cargo run --example gtk_cookies
cargo run --example gtk_custom_protocol --features protocol
cargo run --example gtk_async_custom_protocol --features protocol
cargo run --example gtk_streaming --features protocol
cargo run --example gtk_transparent
cargo run --example gtk_custom_titlebar
cargo run --example gtk_window_border
cargo run --example gtk_permission_handler
cargo run --example gtk_linux_features
cargo run --example reparent
```

### X11 — winit

```bash
cargo run --example simple
cargo run --example multiwebview
cargo run --example multiwindow
cargo run --example cookies
cargo run --example custom_protocol --features protocol
cargo run --example async_custom_protocol --features protocol
cargo run --example streaming --features protocol
cargo run --example transparent
cargo run --example window_border
cargo run --example custom_titlebar
cargo run --example permission_handler
cargo run --example wgpu
cargo run --example winit
```

---

## Feature Notes

### IME / CJK Input (Fcitx5, IBus)

GTK4 routes input-method events through the `GtkEventControllerKey` pipeline. wry no longer
calls `set_enable_preedit(false)` — a workaround that was introduced for the GTK3/WebKitGTK
cursor-anchor bug (WebKit bug 218148) where the IME popup drifted to the top-left corner of
the screen. That bug was fixed upstream in WebKitGTK 2.44 (May 2024). Removing the workaround
restores inline preedit composition (the composing-character preview) for all CJK users on
both Fcitx5 and IBus, and eliminates the Fcitx/Fcitx5 regression where the first character
of subsequent words was silently dropped when preedit was disabled (Mozilla bug #1742039).

No special configuration is required — Fcitx5 and IBus inline composition work out of the
box on webkitgtk-6.0 ≥ 2.44.

---

### Wayland Native Embedding

Enable with `--features wayland` (mirrors the existing `x11` feature). Both can be active
simultaneously — the backend dispatches at runtime on the window handle type.

```toml
# Cargo.toml
wry = { version = "...", features = ["wayland"] }
```

**How it works:**

`WebViewBuilder::new()` and `WebViewBuilder::new_as_child()` accept a
`RawWindowHandle::Wayland` handle. The backend locates the `GtkWindow` that owns the
given `wl_surface` by iterating `gtk::Window::list_toplevels()` and comparing raw surface
pointers via `gdk_wayland_surface_get_wl_surface`. This means **the parent window must
be a realized GTK4 window** — a raw winit or foreign-toolkit surface will not be found
and will return `Error::WaylandWindowNotFound`. Common causes of this error: the
`wl_surface` comes from a non-GTK toolkit, `gtk4::init()` was not called before creating
the webview, or the GTK4 window was not yet realized/shown.

**Child mode** (`new_as_child`): finds-or-creates a `GtkFixed` as the root window's child
widget and places the WebView at `bounds.position`. `set_bounds()` uses `GtkFixed::move_`
+ `set_size_request`; `bounds()` returns the last-set position plus `widget.allocation()`
for size.

**Non-child mode** (`new`): replaces the root window's child with a `GtkBox` and expands
the WebView to fill it.

**Example — child embed at a fixed rect:**

```rust
use wry::WebViewBuilder;
use raw_window_handle::HasWindowHandle;

// `parent` is any type that returns RawWindowHandle::Wayland —
// e.g. a gtk4::ApplicationWindow exposed via WindowHandle
let webview = WebViewBuilder::new_as_child(&parent)
    .with_bounds(wry::Rect {
        position: dpi::LogicalPosition::new(10, 10).into(),
        size:     dpi::LogicalSize::new(800, 600).into(),
    })
    .with_url("https://example.com")
    .build()?;
```

**HiDPI / Scaling:**

Scale-factor conversion in `set_bounds()` and `bounds()` uses `gdk4::SurfaceExt::scale()`
which returns a fractional `f64` (available since GDK 4.12). The `gdk4/v4_12` feature is
always enabled, so fractional-scale compositors (e.g. GNOME 45+ at 1.25×, 1.5×) are handled
correctly. On systems running GDK < 4.12 the library falls back to the integer
`scale_factor`, which is exact for 1× and 2× scaling.

**Limitations:**

| Limitation | Reason |
|---|---|
| Parent must be a GTK4 window | `wl_surface` lookup iterates GTK toplevels only |
| Cross-process surface embedding | Wayland `xdg-foreign-unstable-v2` has no GTK4 binding |
| `wl_subsurface` for non-GTK parents | GDK4 does not expose `wl_subcompositor` publicly |
| winit surfaces without GTK display backend | `wl_surface` not registered in the GDK display |

For Wayland-capable apps that already own GTK4 windows, use `build_gtk` directly —
it avoids the surface-lookup overhead and works on both X11 and Wayland without a feature flag.

---

### Advancing the GTK Event Loop

When integrating wry with a non-GTK windowing library (e.g. winit), GLib/GTK events must
be drained once per outer-loop iteration. The helper `wry::pump_platform_events()` does this:

```rust
fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    wry::pump_platform_events();
}
```

Without this call, network requests, rendering callbacks, IPC messages, and JavaScript
timers will stall. Internally it is equivalent to:

```rust
// Manual equivalent — shown for reference only
while gtk4::glib::MainContext::default().iteration(false) {}
```

Use `wry::pump_platform_events()` in preference to the manual form; it is exported only on
Linux/BSD (the `#[cfg(gtk)]` target) so no `#[cfg]` guard is needed in cross-platform code
when targeting only those platforms, but you will still need `#[cfg(target_os = "linux")]`
if your binary must also compile on Windows/macOS.

---

### Streaming

Both `gtk_streaming` and `streaming` share `examples/streaming/index.html`, which has a
**Local file / Remote HTTPS** toggle in the top bar.

**Local file** — enter an absolute path to any video file and press **Play**. The file
is served over the custom `stream://` protocol with HTTP Range support. The MIME type is
detected automatically from the extension:

| MIME type | Extensions |
|-----------|-----------|
| `video/mp4` | `.mp4` `.m4v` `.m4p` |
| `video/webm` | `.webm` |
| `video/ogg` | `.ogg` `.ogv` |
| `video/quicktime` | `.mov` `.qt` |
| `video/x-matroska` | `.mkv` `.mk3d` |
| `video/x-msvideo` | `.avi` |
| `video/x-flv` | `.flv` `.f4v` |
| `video/x-ms-wmv` | `.wmv` `.asf` |
| `video/mpeg` | `.mpeg` `.mpg` `.mpe` `.m2v` `.m1v` |
| `video/mp2t` | `.ts` `.m2ts` `.mts` |
| `video/3gpp` | `.3gp` `.3gpp` |
| `video/3gpp2` | `.3g2` `.3gpp2` |
| `video/hevc` | `.hevc` `.h265` |
| `application/mxf` | `.mxf` |
| `application/vnd.rn-realmedia` | `.rm` `.rmvb` |

> Whether WebKit *plays* a format depends on what GStreamer codecs are installed — the
> MIME type tells the browser what it is receiving, GStreamer does the decoding. Install
> extra codec packs (e.g. `gstreamer1.0-plugins-bad`, `gstreamer1.0-libav`) to broaden
> support.

**Remote HTTPS** — switch to the *Remote HTTPS* tab to stream 10-second Big Buck Bunny
test clips directly over HTTPS from [test-videos.co.uk](https://test-videos.co.uk)
without involving the `stream://` protocol.

| Format | Codec | Resolutions |
|--------|-------|-------------|
| MP4 | H.264 | 360p · 720p · 1080p |
| MP4 | AV1 | 360p · 720p · 1080p |
| WebM | VP9 | 360p · 720p · 1080p |

*Big Buck Bunny* (c) 2008 Blender Foundation — [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/).

---

### Permission Handler

`gtk_permission_handler` demonstrates wry's `with_permission_handler` API, which maps
webkit6 permission requests to `PermissionKind` values and returns `Allow`, `Deny`, or
`Default`.

**`Default` on Linux = Deny.** webkit6 has no native permission prompt on Linux, so
`PermissionResponse::Default` is equivalent to `Deny`.

**Granting a permission does not guarantee the feature works.** webkit6 delegates to
OS-level services that must be present on the host:

| PermissionKind | webkit6 signal | System service required | macOS support |
|---|---|---|---|
| `Geolocation` | `GeolocationPermissionRequest` | **geoclue2** | ✓ macOS 12+ (`requestGeolocationPermissionForOrigin`) |
| `Camera` | `UserMediaPermissionRequest` (video) | **PipeWire** + v4l2 camera | ✓ (`requestMediaCapturePermission`) |
| `Microphone` | `UserMediaPermissionRequest` (audio) | **PipeWire** or PulseAudio | ✓ (`requestMediaCapturePermission`) |
| `Sensors` | — | — | ✓ (`requestDeviceOrientationAndMotionPermission`) |
| `Notifications` | `NotificationPermissionRequest` | Desktop notification daemon | ✗ (system dialog, no WKUIDelegate hook) |
| `ClipboardRead` | `ClipboardPermissionRequest` | None — compositor | ✗ (no WKUIDelegate hook) |
| `PointerLock` | `PointerLockPermissionRequest` | None — compositor | ✗ (no WKUIDelegate hook) |
| `MediaKeySystemAccess` | `MediaKeySystemPermissionRequest` | **Widevine** (closed-source) | ✗ |

Typical errors when the service is absent:

| Feature | Error |
|---|---|
| Geolocation | `Failed to connect to geolocation service` |
| Camera / Mic | `Could not start video source` / `getUserMedia() failed` |
| MediaKeySystem | `Unsupported key system` |

**Camera and Microphone** are disabled in webkit6 by default. Enable them via the builder:

```rust
let webview = wry::WebViewBuilder::new()
    .with_enable_media_stream(true)    // Camera + Microphone
    .with_enable_encrypted_media(true) // MediaKeySystem (EME/DRM)
    // ...
    .build_gtk(&vbox)?;
```

**Geolocation** requires geoclue2 (Ubuntu/Debian: `sudo apt install geoclue-2.0`). On
GNOME it starts automatically via D-Bus; on other desktops it may need to be started
manually.

### Theme / Dark Mode

> **Example:** `cargo run --example gtk_linux_features` — press **T** to toggle Dark/Light at runtime.

Use `with_theme` (from `WebViewBuilderExtUnix`) to control the `prefers-color-scheme`
CSS media feature:

```rust
use wry::{WebViewBuilderExtUnix, Theme};

let webview = WebViewBuilder::new()
    .with_theme(Theme::Dark)   // force prefers-color-scheme: dark
    // .with_theme(Theme::Light)  // force prefers-color-scheme: light
    // .with_theme(Theme::Auto)   // follow system (default)
    .build_gtk(&vbox)?;
```

WebKitGTK reads GTK4's `gtk-application-prefer-dark-theme` display setting to set
`prefers-color-scheme`. `Theme::Auto` is a no-op — the system/desktop preference is used.

> **Warning — process-wide side effect.** `with_theme` calls
> `gtk::Settings::set_gtk_application_prefer_dark_theme()`, which is a **GDK
> display-level** property. It affects every GTK widget in the process for the
> lifetime of the display — not just the webview being built. If you create
> multiple webviews with different themes, the last call wins for the entire
> process. This is the intended behaviour for most wry applications (a single
> fullscreen webview per process), but be aware of the side effect when mixing
> native GTK widgets with wry webviews.

**Per-page colour scheme without the display-wide effect**

If you need to override `prefers-color-scheme` for a single page without changing the
GTK display setting, inject a CSS init script instead:

```rust
let webview = WebViewBuilder::new()
    .with_initialization_script(
        "document.documentElement.style.colorScheme = 'dark';"
    )
    .build_gtk(&vbox)?;
```

This sets the CSS `color-scheme` property on the root element, which causes most
frameworks (Tailwind, shadcn/ui, …) to switch to dark mode via their own CSS variables,
without touching any GTK setting.

---

### Focus Handler

Use `with_focus_handler` (from `WebViewBuilderExtUnix`) to be notified when the webview
gains or loses keyboard focus:

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_focus_handler(|focused| {
        if focused {
            println!("webview gained focus");
        } else {
            println!("webview lost focus");
        }
    })
    .build_gtk(&vbox)?;
```

The handler receives `true` on focus-in and `false` on focus-out. It is implemented via
GTK4's `GtkEventControllerFocus` and is Linux/BSD only.

---

### Keyboard Handler

Use `with_keyboard_handler` (from `WebViewBuilderExtUnix`) to intercept key-press events
before WebKit processes them:

```rust
use wry::WebViewBuilderExtUnix;
use webkit6::gdk::ModifierType;

let webview = WebViewBuilder::new()
    .with_keyboard_handler(|keyval, _keycode, modifiers| {
        if modifiers.contains(ModifierType::CONTROL_MASK) && keyval == b'r' as u32 {
            println!("Ctrl+R intercepted");
            return true; // consume — WebKit does not see this event
        }
        false // propagate normally
    })
    .build_gtk(&vbox)?;
```

Return `true` to consume the event (WebKit will not receive it); return `false` to let it
propagate. The `keyval` is the raw GDK keysym as a `u32`. Linux/BSD only.

---

### Clipboard

**Write** (synchronous, no JS required):

```rust
use wry::WebViewExtUnix;

webview.write_clipboard_text("hello from wry");
```

**Read** (asynchronous callback, fires on the GLib main context):

```rust
use wry::WebViewExtUnix;

webview.read_clipboard_text(|text| {
    match text {
        Some(s) => println!("clipboard: {s}"),
        None    => println!("clipboard empty or unavailable"),
    }
});
```

Neither method requires `with_clipboard(true)` or a JS round-trip. Linux/BSD only.

---

### Drag Source (outgoing drags)

Use `with_drag_source_handler` (from `WebViewBuilderExtUnix`) to let the webview initiate
drags to other applications:

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_drag_source_handler(|x, y| {
        println!("drag started at ({x}, {y})");
        Some("dragged text content".to_string()) // None cancels the drag
    })
    .build_gtk(&vbox)?;
```

The handler receives the pointer position where the drag began and should return the text
to drag, or `None` to cancel. Implemented via GTK4's `GtkDragSource`. Linux/BSD only;
text-only in this initial implementation.

---

### Pointer Motion / Enter / Leave

Use `WebViewBuilderExtUnix` to receive pointer-motion and crossing events from the webview
widget without injecting JavaScript. All three handlers are independent and optional.

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_motion_handler(|x, y| {
        println!("pointer at ({x:.1}, {y:.1})");
    })
    .with_pointer_enter_handler(|x, y| {
        println!("pointer entered at ({x:.1}, {y:.1})");
    })
    .with_pointer_leave_handler(|| {
        println!("pointer left webview");
    })
    .build_gtk(&vbox)?;
```

Coordinates are in widget-local logical pixels. Implemented via `GtkEventControllerMotion`.
**Linux/BSD only.**

---

### Scroll Interception

Use `WebViewBuilderExtUnix::with_scroll_handler` to intercept scroll events before WebKit
processes them. Return `true` to consume (suppress) the event; `false` to let it pass through.

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_scroll_handler(|dx, dy| {
        println!("scroll delta ({dx:.2}, {dy:.2})");
        false // let WebKit handle it
    })
    .build_gtk(&vbox)?;
```

`delta_x` and `delta_y` are in scroll units (positive = right/down). Useful for implementing
custom scroll-to-zoom or intercepting horizontal scroll for tab switching. Implemented via
`GtkEventControllerScroll` with `BOTH_AXES`. **Linux/BSD only.**

---

### Cursor Control

Use `WebViewExtUnix::set_cursor_from_name` to override the GTK cursor shown over the webview
widget, independently of any CSS `cursor:` properties on the page:

```rust
use wry::WebViewExtUnix;

// set a custom cursor
webview.set_cursor_from_name(Some("crosshair"));

// restore default browser cursor
webview.set_cursor_from_name(None);
```

Accepts any named CSS cursor string (`"grab"`, `"zoom-in"`, `"not-allowed"`, etc.).
Uses `gtk::Widget::set_cursor_from_name` (GTK 4.0+). **Linux/BSD only.**

---

### Primary Clipboard (X11 Selection)

Use `WebViewExtUnix` to read and write the X11 primary selection — the clipboard
populated on text selection and pasted with middle-click:

```rust
use wry::WebViewExtUnix;

// write to primary selection
webview.write_primary_clipboard_text("selected text");

// read from primary selection (async)
webview.read_primary_clipboard_text(|text| {
    println!("primary: {:?}", text);
});
```

On Wayland compositors, write access is restricted by the compositor security policy and the
read callback will typically receive `None`. Uses `GdkDisplay::primary_clipboard`.
**Linux/BSD only.**

---

### Monitor Change Notifications

Use `WebViewBuilderExtUnix::with_monitors_changed_handler` to be notified when the monitor
configuration changes (monitor connected, disconnected, or reconfigured):

```rust
use wry::{WebViewBuilderExtUnix, MonitorInfo};

let webview = WebViewBuilder::new()
    .with_monitors_changed_handler(|monitors: Vec<MonitorInfo>| {
        for m in &monitors {
            println!(
                "monitor {:?}: {:?} @{}x",
                m.model, m.geometry, m.scale_factor
            );
        }
    })
    .build_gtk(&vbox)?;
```

`MonitorInfo` exposes `geometry` (logical-pixel `Rect`), `scale_factor` (`i32`), and
`model` (`Option<String>`). The handler fires on `GListModel::items-changed` from
`GdkDisplay::monitors()`. **Linux/BSD only.**

---

### Web Process Crash Handler

> **Example:** `cargo run --example gtk_linux_features` — press **C** to crash the web process and see the handler fire.

Use `with_on_web_content_process_terminate_handler` (from `WebViewBuilderExtUnix`) to be
notified when the WebKit web process crashes or is killed due to exceeding memory limits:

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_on_web_content_process_terminate_handler(|| {
        eprintln!("web process terminated — reloading");
    })
    .build_gtk(&vbox)?;
```

If you need the termination reason (`Crashed` / `ExceededMemoryLimit`), connect the
underlying webkit6 signal directly:

```rust
use wry::WebViewExtUnix;
use webkit6::prelude::WebViewExt;

webview.webview().connect_web_process_terminated(|_, reason| {
    eprintln!("terminated: {reason:?}");
});
```

---

### New Window Requests

Use `WebViewBuilder::with_new_window_req_handler` to intercept `window.open()` calls and
`<a target="_blank">` navigations. The handler receives a `NewWindowFeatures` struct that
includes a `NewWindowOpener` with a `webview: WebViewHandle` field pointing to the opener.

On Linux, pass `opener.webview` to `WebViewBuilderExtUnix::with_related_view` so the new
webview shares the same WebKit web process as the opener:

```rust
use wry::{WebViewBuilder, WebViewBuilderExtUnix, NewWindowResponse};

let webview = WebViewBuilder::new()
    .with_url("https://tauri.app")
    .with_new_window_req_handler(|url, features| {
        let opener = features.opener;
        // Spawn the new window, then build the webview sharing the opener's web process
        let vbox = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let window = gtk4::Window::builder().child(&vbox).build();
        window.present();

        let new_webview = WebViewBuilder::new()
            .with_url(&url)
            .with_related_view(opener.webview) // share web process with opener
            .build_gtk(&vbox)
            .unwrap();

        NewWindowResponse::Deny // wry won't open its own window; we handled it above
    })
    .build_gtk(&vbox)?;
```

To hand the new webview back to WebKit instead (letting it manage the relationship), use
`NewWindowResponse::Create { webview: handle }` where `handle` is the `WebViewHandle` of the
newly-built webview:

```rust
NewWindowResponse::Create { webview: new_webview.into_handle() }
```

> **Note:** `opener.webview` holds a `WebViewHandle` — an opaque wrapper around the
> platform webview. On Linux you can access the underlying `webkit6::WebView` via the
> `WebViewHandleExtUnix` trait: `opener.webview.as_webkit6_webview()`.

---

### Isolated Data Directory

> **Example:** `cargo run --example gtk_linux_features` — check stdout for the resolved path under `/tmp/wry-gtk-features-demo`.

Use `WebViewBuilderExtUnix::with_data_directory()` to give a webview its own isolated storage
directory instead of the shared WebKit default:

```rust
use wry::{WebViewBuilder, WebViewBuilderExtUnix};

let webview = WebViewBuilder::new()
    .with_url("https://example.com")
    .with_data_directory("/var/app/profile-a")
    .build_gtk(&vbox)?;
```

All WebKit persistent data for this webview (cache, IndexedDB, localStorage, cookies) will be
stored under the given path. This is the Linux equivalent of `with_profile_name` (Windows) and
`with_data_store_identifier` (macOS). If you also pass an explicit `WebContext` via
`with_web_context`, the data directory on the context takes precedence and `with_data_directory`
is ignored.

---

### Cookie Accept Policy

> **Example:** `cargo run --example gtk_cookies` — runs with `CookieAcceptPolicy::Never`; HTTP Set-Cookie headers are blocked while programmatic cookies still work.

Use `WebContext::set_cookie_accept_policy()` to control which cookies WebKit will accept for
all webviews sharing that context:

```rust
use wry::WebContext;

let mut context = WebContext::new(None);
context.set_cookie_accept_policy(webkit6::CookieAcceptPolicy::NoThirdParty);
```

The three variants mirror the underlying WebKitGTK enum:

| Variant | Behaviour |
|---|---|
| `CookieAcceptPolicy::Always` | Accept all cookies (WebKit default) |
| `CookieAcceptPolicy::Never` | Reject all cookies |
| `CookieAcceptPolicy::NoThirdParty` | Accept only first-party cookies |

#### Cookie API — blocking behaviour on Linux

`WebView::cookies()`, `WebView::cookies_for_url()`, `WebView::set_cookie()`, and
`WebView::delete_cookie()` use `glib::MainContext::block_on` to drive the async WebKit
cookie operation to completion. This is the same mechanism GTK uses for modal dialogs —
a nested `GMainLoop` is started on the current main context and runs until the future
resolves, then exits cleanly.

- **They block the calling thread** until the WebKit network process responds. For typical
  cookie operations this is imperceptible.
- **They must be called from the GLib main-context thread** (i.e. the UI / main thread).
  Calling them from a thread that does not own the default main context will panic.
- **Calling them from inside a GLib callback is safe** — `block_on` starts a nested event
  loop, so other GLib sources (UI events, IPC, timers) continue to be dispatched while
  waiting. This is identical to how GTK modal dialogs behave.

If you need cookie access from a background thread, dispatch the call to the main thread
using `glib::MainContext::default().spawn()` and receive the result via a channel.

---

### Data Directory Accessor

> **Example:** `cargo run --example gtk_linux_features` — the accessor result is printed to stdout on startup.

Use `WebViewExtUnix::data_directory()` to retrieve the base data directory path that the
underlying `NetworkSession` was initialised with:

```rust
use wry::WebViewExtUnix;

if let Some(dir) = webview.data_directory() {
    println!("WebKit data stored at: {}", dir.display());
    // enumerate or remove files under dir as needed
}
```

Returns `None` when the webview uses a default or ephemeral (incognito) session that has no
custom data directory. On Linux, all per-origin storage (cache, IndexedDB, cookies, …) lives
under this path, so listing or deleting it is the equivalent of the macOS
`fetch_data_store_identifiers` / `remove_data_store` APIs.

---

## Known Issues

### GLIBC_PRIVATE symbol error

**Symptom:**

```
/usr/lib/x86_64-linux-gnu/webkitgtk-6.0/WebKitNetworkProcess: symbol lookup error:
/snap/core20/current/lib/x86_64-linux-gnu/libpthread.so.0: undefined symbol:
__libc_pthread_init, version GLIBC_PRIVATE
```

**Cause:** Snap applications inject `SNAP_LIBRARY_PATH` into the environment. The
sandboxed `WebKitNetworkProcess` inherits it and resolves `libpthread` from the snap's
Ubuntu 20.04 base, which is ABI-incompatible with the system glibc.

---

### NeedDebuggerBreak trap in stderr

**Symptom:**

```
VM 0x7f... on pid ... received NeedDebuggerBreak trap
```

**Cause:** JavaScriptCore prints this when the JS VM hits a debug-trap with no native
debugger attached. It is **not an error** — the webview continues normally. Only appears
in debug builds; `cargo run --release` will not produce it.

---

### DMA-BUF rendering issues

**Symptom:** Blank webview, GPU crash, or rendering corruption in Docker, CI runners, or
with certain Mesa/Wayland/EGL driver stacks.

**Cause:** WebKitGTK defaults to DMA-BUF GPU buffers, which requires a working EGL/DRM
stack. Headless environments and some driver configurations do not support this.

**Fix (programmatic):** Use `with_hardware_acceleration_policy` from `WebViewBuilderExtUnix`
to force software rendering for the webview (see also `gtk_linux_features` example):

```rust
use wry::WebViewBuilderExtUnix;
use webkit6::HardwareAccelerationPolicy;

let webview = WebViewBuilder::new()
    .with_hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
    .build_gtk(&vbox)?;
```

**Fix (environment variable):** Set before launching the process:

```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo run --example gtk_simple
```

# Migrating to gtk4/webkit6

Welcome to the migration guide for updating `wry` from `gtk`/`webkit2gtk` to
`gtk4`/`webkit6`. This guide walks you through everything you'll need to change.

The sections below cover everything you need to update, roughly in the order you'll
hit them — system packages first, then `Cargo.toml`, then the API changes.

---

## 1. System dependencies

Replace the old webkit2gtk development packages with their GTK 4 / WebKitGTK 6
equivalents before building.

| Distro | Remove | Install |
|---|---|---|
| Debian / Ubuntu | `libwebkit2gtk-4.1-dev` | `libwebkitgtk-6.0-dev libgtk-4-dev` |
| Arch / Manjaro | `webkit2gtk-4.1` | `webkitgtk-6.0` |
| Fedora | `gtk3-devel webkit2gtk4.1-devel` | `gtk4-devel webkitgtk6.0-devel` |
| Nix / NixOS | `webkitgtk_4_1` | `webkitgtk_6_0` |

---

## 2. `Cargo.toml` changes

### Replace webkit2gtk / gtk with webkit6 / gtk4

```toml
# Before
webkit2gtk = { version = "2", features = ["v2_38"] }
gtk = "0.18"

# After
webkit6 = "0.6"
gtk4 = "0.11"       # only if you import gtk types directly
```

If you depend on `webkit6` directly (not just through wry's re-export), match wry's
feature gate: `webkit6 = { version = "0.6", features = ["v2_42"] }`.

### Remove the `linux-body` feature opt-in

`linux-body` is now part of the default feature set. Explicit opt-in can be removed.

```toml
# Before
wry = { version = "...", features = ["linux-body"] }

# After
wry = { version = "..." }
```

Previously `linux-body` was opt-in, which caused `request.body()` to silently return
an empty slice on Linux while Windows and macOS always returned the full body. The
gtk4/webkit6 backend requires WebKitGTK 6.x (above the v2.40 minimum `linux-body`
needs), so the feature is now always available and enabled by default.

---

## 3. GTK initialisation and event loop

### Init call

```rust
// Before
gtk::init().unwrap();

// After
gtk4::init().unwrap();
```

### Draining the GLib event loop

When integrating wry with a non-GTK windowing library, GLib/GTK events must be drained
once per outer-loop iteration. Use the `wry::pump_platform_events()` helper instead of
the old manual pattern:

```rust
// Before
while gtk::main_iteration_do(false) {}

// After — inside your event loop's about_to_wait / idle callback
wry::pump_platform_events();
```

`wry::pump_platform_events()` is exported only on Linux/BSD so no `#[cfg]` guard is
needed when targeting only those platforms. You still need `#[cfg(target_os = "linux")]`
if your binary must also compile on Windows/macOS.

---

## 4. Builder API: `build_gtk` now takes a `gtk4::Widget`

`WebViewBuilderExtUnix::build_gtk` is not a new method — it already existed on the
`dev` branch. What changed is the trait bound on its container argument, because
GTK4 removed `Container` as a base widget class:

```rust
// Before (dev, GTK3) — accepts a gtk::Container
let webview = WebViewBuilder::new()
    .with_url("https://example.com")
    .build_gtk(&vbox)?;   // vbox: impl gtk::prelude::IsA<gtk::Container>

// After (gtk4-webkit6) — same method, now accepts a gtk4::Widget
let webview = WebViewBuilder::new()
    .with_url("https://example.com")
    .build_gtk(&vbox)?;   // vbox: impl webkit6::gtk::prelude::IsA<webkit6::gtk::Widget>
```

### GTK4 concepts new to GTK3 users

GTK4 collapsed `Container`/`Bin`/`Widget` into a single `Widget` hierarchy with
different child-management APIs. If your application implements a custom GTK
container or calls `gtk::prelude::ContainerExt` methods (`add`, `remove`,
`child-type`, …) directly, you'll need to port those call sites to the GTK4
equivalents (`append`/`prepend` on `gtk4::Box`, `set_child` on single-child
widgets like `gtk4::Window` or `gtk4::ScrolledWindow`, etc.) as part of porting to
`gtk4-rs`. See the [upstream GTK4 migration guide](https://docs.gtk.org/gtk4/migrating-3to4.html)
if you maintain custom widgets.

There is also a separate, unrelated convenience method — `WebViewExtUnix::new_gtk(widget)`
— which constructs a `WebView` directly (`WebViewBuilder::new().build_gtk(widget)`
under the hood) without going through the builder. It is unchanged between `dev`
and this branch.

---

## 5. Reparent rename: `reparent` → `reparent_gtk`

`WebViewExtUnix::reparent` has been renamed to `reparent_gtk`.

```rust
// Before
use wry::WebViewExtUnix;
webview.reparent(&vbox2)?;

// After
use wry::WebViewExtUnix;
webview.reparent_gtk(&vbox2)?;
```

**Why:** A new cross-platform `WebView::reparent(&impl HasWindowHandle)` inherent
method was added. Inherent methods shadow trait methods in Rust, so the old name would
silently compile to the wrong call. The `_gtk` suffix removes the ambiguity and makes
the intent explicit.

### New cross-platform reparent

For webviews created via `WebViewBuilder::build` (X11 raw-handle mode), the new
inherent method is the right choice:

```rust
// Works on Windows, macOS, Linux X11
webview.reparent(&other_window)?;   // other_window: impl HasWindowHandle
```

Use `reparent_gtk` for webviews created via `build_gtk`; use `reparent` for webviews
created via `build`.

---

## 6. `WebViewHandle` API changes

### Opaque handle replaces raw platform type

On `dev`, `NewWindowOpener.webview` and `NewWindowResponse::Create { webview }` held
a raw `webkit2gtk::WebView` directly — no accessor was needed. This branch wraps the
platform webview in an opaque `WebViewHandle`, accessed via `WebViewHandleExtUnix`
(consistent with `as_core_webview2` on Windows and `as_wk_webview` on macOS):

```rust
// Before (dev) — raw field access
let view: webkit2gtk::WebView = opener.webview;

// After (gtk4-webkit6) — opaque handle, requires the extension trait
use wry::WebViewHandleExtUnix;
let view: &webkit6::WebView = opener.webview.as_webkit_webview();
let view: webkit6::WebView  = opener.webview.into_webkit_webview();
```

### New constructor: `WebViewHandle::from_webkit_webview`

If you construct a `webkit6::WebView` outside of `WebViewBuilder` and need to pass it
to `NewWindowResponse::Create` or `with_related_view`, wrap it with the new constructor:

```rust
use wry::{WebViewHandle, NewWindowResponse};

let raw: webkit6::WebView = /* externally constructed */;
NewWindowResponse::Create {
    webview: WebViewHandle::from_webkit_webview(raw),
}
```

### `NewWindowOpener` / `NewWindowResponse` now hold `WebViewHandle`

These fields previously held a raw platform type. They now hold the opaque
`WebViewHandle`. Access the underlying view through the extension trait:

```rust
use wry::WebViewHandleExtUnix;

let raw: &webkit6::WebView = opener.webview.as_webkit_webview();
```

### `with_related_view` now takes a `WebViewHandle`

`WebViewBuilderExtUnix::with_related_view` previously accepted a raw `webkit2gtk::WebView`.
It now accepts a `WebViewHandle`, for the same reason as above.

```rust
// Before
builder.with_related_view(raw_webkit2gtk_view)

// After
use wry::WebViewHandle;
builder.with_related_view(WebViewHandle::from_webkit_webview(raw_webkit6_view))
```

---

## 7. Type and import changes

The most common import paths to update:

| Old | New |
|---|---|
| `webkit2gtk::WebView` | `webkit6::WebView` |
| `webkit2gtk::WebContext` | `webkit6::WebContext` |
| `webkit2gtk::UserContentManager` | `webkit6::UserContentManager` |
| `gtk::prelude::*` | `webkit6::prelude::*` |
| `gtk::glib::MainContext` | `webkit6::glib::MainContext` |
| `gtk::Box` / `gtk::Window` | `gtk4::Box` / `gtk4::Window` |
| `gtk::Orientation` | `gtk4::Orientation` |

**Prelude:** `use webkit6::prelude::*` is the single import that re-exports
`gtk4::prelude::*`, `glib::prelude::*`, `Cast`, `IsA`, and all Ext traits. Do not
import `Cast`, `IsA`, or `PermissionRequestExt` by name from `webkit6::glib` — they
conflict with each other and with the gtk4 re-exports.

---

## 8. X11 and Wayland embedding

### `build_gtk` — recommended, works on both X11 and Wayland

```rust
let webview = WebViewBuilder::new()
    .with_url("https://example.com")
    .build_gtk(&vbox)?;     // vbox: gtk4::Box or any GTK widget
```

No feature flag required. Works on Wayland and X11.

### X11 raw-handle path — unchanged

The `x11` feature is enabled by default (as it was on `dev`) — no `Cargo.toml`
change is needed for this path.

```rust
let webview = WebViewBuilder::new()
    .with_url("https://example.com")
    .build(&window)?;   // window: impl HasWindowHandle (X11 handle)
```

Use `webview.reparent(&other_window)` to move it between X11 windows at runtime.

### Wayland native embedding — new in this branch

Enable with the `wayland` cargo feature. Accepts `RawWindowHandle::Wayland`, but the
parent window **must be a realized GTK4 window** — a foreign-toolkit `wl_surface` is
not found and returns `Error::WaylandWindowNotFound`.

```toml
wry = { version = "...", features = ["wayland"] }
```

```rust
let webview = WebViewBuilder::new_as_child(&parent)
    .with_bounds(wry::Rect {
        position: dpi::LogicalPosition::new(10, 10).into(),
        size:     dpi::LogicalSize::new(800, 600).into(),
    })
    .with_url("https://example.com")
    .build()?;
```

For Wayland apps that already own GTK4 windows, prefer `build_gtk` — it avoids the
surface-lookup overhead and works on X11 as well.

---

## 9. New Linux-only APIs

The following APIs were added in this branch and are available immediately after
migrating. See [`LINUX.md`](LINUX.md) for full documentation and examples.

| Method | Trait | What it does |
|---|---|---|
| `with_scroll_handler` | `WebViewBuilderExtUnix` | Intercept scroll events; return `true` to consume |
| `with_focus_handler` | `WebViewBuilderExtUnix` | Notified on focus-in / focus-out |
| `with_keyboard_handler` | `WebViewBuilderExtUnix` | Intercept key-press before WebKit |
| `write_clipboard_text` / `read_clipboard_text` | `WebViewExtUnix` | Clipboard read/write without JS |
| `write_primary_clipboard_text` / `read_primary_clipboard_text` | `WebViewExtUnix` | X11 primary selection |
| `with_motion_handler` / `with_pointer_enter_handler` / `with_pointer_leave_handler` | `WebViewBuilderExtUnix` | Pointer position and crossing events |
| `set_cursor_from_name` | `WebViewExtUnix` | Override the GTK cursor by CSS name |
| `with_monitors_changed_handler` | `WebViewBuilderExtUnix` | Monitor connect / disconnect / reconfigure |
| `with_on_web_content_process_terminate_handler` | `WebViewBuilderExtUnix` | Web process crash handler |
| `with_drag_source_handler` | `WebViewBuilderExtUnix` | Initiate outgoing drags to other applications |
| `with_data_directory` / `data_directory` | `WebViewBuilderExtUnix` / `WebViewExtUnix` | Isolated per-webview storage directory |
| `with_hardware_acceleration_policy` | `WebViewBuilderExtUnix` | Force software rendering (useful in CI/Docker) |
| `with_theme` | `WebViewBuilderExtUnix` | Control `prefers-color-scheme` (process-wide) |
| `with_enable_media_stream` | `WebViewBuilder` | Enable camera and microphone (off by default in webkit6) |

---

## 10. Behavioral differences

### IME / CJK input composition now works inline

On `dev`, a `set_enable_preedit(false)` workaround was needed to avoid a WebKitGTK
bug that anchored the Fcitx IME popup at (0,0) instead of the cursor position. That
bug was fixed upstream in WebKitGTK 2.44, so this branch removes the workaround.
Fcitx/Fcitx5 and IBus inline preedit composition now work correctly for CJK users.

**If your application added its own workaround** for broken preedit positioning or
dropped first characters during composition, you can likely remove it after
migrating — test CJK input to confirm.

---

## 11. Known issues

### GLIBC_PRIVATE symbol error

```
WebKitNetworkProcess: undefined symbol: __libc_pthread_init, version GLIBC_PRIVATE
```

Caused by Snap injecting `SNAP_LIBRARY_PATH`. The sandboxed `WebKitNetworkProcess`
inherits it and resolves `libpthread` from Snap's Ubuntu 20.04 base, which is
ABI-incompatible with the system glibc. Not a wry bug.

### `NeedDebuggerBreak` trap in stderr

```
VM 0x7f... on pid ... received NeedDebuggerBreak trap
```

JavaScriptCore prints this when the JS VM hits a debug-trap with no native debugger
attached. Not an error — the webview continues normally. Only appears in debug builds.

### DMA-BUF rendering issues

**Symptom:** Blank webview, GPU crash, or rendering corruption in Docker, CI runners,
or with certain Mesa/Wayland/EGL stacks.

**Fix — environment variable:**
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 cargo run --example gtk_simple
```

**Fix — programmatic:**
```rust
use wry::WebViewBuilderExtUnix;
use webkit6::HardwareAccelerationPolicy;

let webview = WebViewBuilder::new()
    .with_hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
    .build_gtk(&vbox)?;
```

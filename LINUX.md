# Linux Development Notes

## Table of Contents

- [Examples](#examples)
  - [Wayland — GTK4 / webkit6](#wayland--gtk4--webkit6)
  - [X11 — winit](#x11--winit)
- [Feature Notes](#feature-notes)
  - [Streaming](#streaming)
  - [Permission Handler](#permission-handler)
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
| All others | winit / X11 | ✓ | ✗ |

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
`prefers-color-scheme`. Because this is display-wide, `with_theme` affects all GTK
widgets in the process. For most wry-based apps (a single fullscreen webview) this is
the desired behaviour. `Theme::Auto` is a no-op — the system/desktop preference is used.

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

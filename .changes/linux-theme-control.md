---
"wry": minor
---

Add `with_theme` to `WebViewBuilderExtUnix` on Linux.

Exposes control over the `prefers-color-scheme` CSS media feature through the Wry
builder, matching the `with_theme` method already available on Windows via
`WebViewBuilderExtWindows`:

```rust
use wry::{WebViewBuilderExtUnix, Theme};

let webview = WebViewBuilder::new()
    .with_theme(Theme::Dark)   // force dark
    // .with_theme(Theme::Light)  // force light
    // .with_theme(Theme::Auto)   // follow system (default, no-op)
    .build_gtk(&vbox)?;
```

On Linux, WebKitGTK reads GTK4's `gtk-application-prefer-dark-theme` display setting
to determine the `prefers-color-scheme` media feature value. `Theme::Dark` sets this
to `true`, `Theme::Light` sets it to `false`, and `Theme::Auto` leaves it unchanged.

Note: this is a display-wide GTK setting and affects all GTK widgets in the process,
not just the webview.

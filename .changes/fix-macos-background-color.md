---
"wry": patch
---

On macOS, implement `background_color` support for WKWebView behind the `transparent` feature. Disables the default white background via the `drawsBackground` KVC key at init and applies `underPageBackgroundColor` on macOS 12+ for both initial creation and runtime `set_background_color` calls.

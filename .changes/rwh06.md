---
"wry": minor
---

**Breaking change** Update [raw-window-handle](https://crates.io/crates/raw-window-handle) crate to v0.6.
- `HasWindowHandle` trait is required for window types instead of `HasRawWindowHandle`.
- `wry::raw_window_handle` now re-exports v0.6.

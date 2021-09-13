---
"tauri": patch
"wry": patch
---

Replace all of the `winapi` crate references with the `windows` crate, and replace `webview2` and `webview2-sys` with `webview2-com` and `webview2-com-sys` built with the `windows` crate. The replacement bindings are in the `webview2-com-sys` crate, with `pub use` in the `webview2-com` crate. They can be shared with TAO.
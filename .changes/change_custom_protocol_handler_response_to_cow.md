---
"wry": "minor"
---

Change return type of [custom protocol handlers](https://docs.rs/wry/latest/wry/webview/struct.WebViewBuilder.html#method.with_custom_protocol) from `Result<Response<Vec<u8>>>` to `Result<Response<Cow<'static, [u8]>>>`. This allows the handlers to return static resources without heap allocations. This is effective when you embed some large files like bundled JavaScript source as `&'static [u8]` using [`include_bytes!`](https://doc.rust-lang.org/std/macro.include_bytes.html).

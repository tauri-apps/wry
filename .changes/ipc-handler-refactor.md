---
"wry": minor
---

Changed `WebViewBuilder::with_ipc_handler` closure to take `http::Request` instead of `String` so the request URL is available.

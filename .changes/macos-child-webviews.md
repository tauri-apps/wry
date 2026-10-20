---
"wry": minor
---

Add `with_new_window_request_handler`, `build_unattached`, and `with_on_webview_close_handler` to `WebViewBuilderExtMacos`. Related macOS Wry webviews route IPC and custom-protocol requests through their own view identity and remain usable after their source view closes.

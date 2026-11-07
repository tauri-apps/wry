---
wry: minor
---

On Windows, add `WebViewBuilderExtWindows::with_frame_inset` and `WebView::set_frame_inset` so a webview filling an undecorated window can leave the DWM border lines uncovered on Windows 10. Defaults to zero; existing apps are unaffected.

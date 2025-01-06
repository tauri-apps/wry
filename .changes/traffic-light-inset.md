---
wry: patch
---

Add functionality to set the traffic light inset on macOS. This is required to prevent flickers if the WebView is injected via `build()` instead of `build_as_child()`.

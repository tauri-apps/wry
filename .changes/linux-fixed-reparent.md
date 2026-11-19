---
wry: patch
---

On Linux, `WebView::reparent` keeps the position of a webview that moves from one `gtk::Fixed` to another. It put the webview at `(0, 0)` before.

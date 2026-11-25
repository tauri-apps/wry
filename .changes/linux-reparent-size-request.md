---
wry: patch
---

On Linux, a webview that `WebView::reparent` moves from a `gtk::Fixed` into another container can shrink below the size it had in the `gtk::Fixed`. That size was kept as its minimum size before.

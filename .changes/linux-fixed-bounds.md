---
wry: patch
---

On Linux, a webview in a `gtk::Fixed` keeps the position and size that `WebView::set_bounds` gave it. A layout pass of the `gtk::Fixed` no longer undoes them, and the position is relative to the `gtk::Fixed`. `WebView::bounds` returns the position and size. This is also true for a webview that `WebView::reparent` moves into a `gtk::Fixed`.

---
"wry": "minor"
---

This changes the Window held by WebView from `Rc<Window>` to `Arc<Window>`, so that the window can be
accessed from other threads. This also changes the `WebView.window` method to return `&Arc<Window>` instead
of `&Window`, which I think should be backwards compatible.

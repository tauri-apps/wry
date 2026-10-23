---
"wry": major
---

Rename `WebViewExtUnix::reparent` to `WebViewExtUnix::reparent_gtk` on Linux/BSD.

The previous name collided with the inherent `WebView::reparent` method (which takes a
`HasWindowHandle`). Because inherent methods shadow trait methods in Rust, calling
`view.reparent(&gtk_widget)` would resolve to the inherent method and fail to compile,
forcing the verbose UFCS form `WebViewExtUnix::reparent(&view, &widget)`.

Renaming to `reparent_gtk` removes the ambiguity — `view.reparent_gtk(&vbox)` now works
directly without any disambiguation syntax, and makes the intent explicit: this variant
moves the webview to a different GTK container.

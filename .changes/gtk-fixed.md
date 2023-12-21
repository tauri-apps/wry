---
"wry": "patch"
---

On Linux, apply passed webview bounds when using `WebView::new_gtk` or `WebViewBuilder::new_gtk` with `gtk::Fixed` widget. This allows to create multiple webviews inside `gtk::Fixed` in the same window.

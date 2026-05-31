---
"wry": minor
---

Add `WebViewBuilderExtWindows::with_native_window_occlusion` to optionally disable WebView2's native window occlusion detection, so a webview keeps rendering while its window is hidden or occluded. Useful for apps that pre-create and show/hide multiple webviews.

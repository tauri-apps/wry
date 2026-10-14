---
"wry": patch
---

Fix macOS panics when `WKWebView.URL()` or the navigation request URL is nil: `url_from_webview()` now returns an error instead of unwrapping, and `navigation_policy()` cancels navigations with no URL.

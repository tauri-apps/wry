---
"wry": patch
---

On Android, keep `RustWebView.getCookies` and `RustWebView.clearAllBrowsingData` in the ProGuard rules so `WebView::cookies_for_url` and `WebView::clear_all_browsing_data` don't abort minified release builds.

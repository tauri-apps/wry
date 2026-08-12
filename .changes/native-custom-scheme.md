---
"wry": minor
---

On Windows, add `WebViewBuilderExtWindows::with_native_custom_scheme` to use native WebView2 custom scheme registration (Runtime >= 110.0.1587.40), keeping custom protocol URLs in their original format instead of rewriting them. Also add `supports_native_custom_scheme()` to check Runtime support.

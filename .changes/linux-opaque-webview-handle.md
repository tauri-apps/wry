---
"wry": major
---

Introduce an opaque `WebViewHandle` type that wraps the platform-specific webview instance. `NewWindowOpener::webview` and `NewWindowResponse::Create { webview }` keep the `webview` field name but now hold a `WebViewHandle` instead of a raw platform type, and `WebViewBuilderExtUnix::with_related_view` now accepts a `WebViewHandle` instead of `webkit6::WebView`. The underlying platform types remain accessible through new extension traits: `WebViewHandleExtUnix` (Linux/BSD), `WebViewHandleExtWindows` (Windows), and `WebViewHandleExtDarwin` (macOS).

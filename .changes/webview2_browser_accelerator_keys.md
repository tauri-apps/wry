---
"wry": "patch"
---

Add `WebViewBuilderExtWindows::with_browser_accelerator_keys` method to allow disabling browser-specific accelerator keys enabled in WebView2 by default. When `false` is passed, it disables all accelerator keys that access features specific to a web browser. See [the official WebView2 document](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings#arebrowseracceleratorkeysenabled) for more details.

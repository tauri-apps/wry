---
"wry": minor
---

Add `WebViewBuilderExtWindows::with_native_custom_scheme` and `with_native_custom_scheme_origins` to use `ICoreWebView2CustomSchemeRegistration` on Windows. `with_native_custom_scheme_origins` allows configuring allowed origins per scheme. Also add `supports_native_custom_scheme()` to check if the installed WebView2 Runtime supports the feature. When enabled and WebView2 Runtime >= 110.0.1587.40, custom protocol URLs keep their original format (e.g. `myscheme://localhost/path`) instead of being rewritten.

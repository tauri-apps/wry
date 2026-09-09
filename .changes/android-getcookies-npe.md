---
"wry": patch
---

**Android:** fix a crash when reading cookies for a URL that has none. `CookieManager.getCookie` returns `null` in that case — a documented, ordinary result — but `RustWebView.getCookies` declared a non-null `String` return type, so Kotlin's intrinsic null check threw an uncaught `NullPointerException` on the main thread and killed the app. It now returns an empty string, which the Rust side already parses into an empty cookie list.

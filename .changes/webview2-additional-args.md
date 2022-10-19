---
"wry": "patch"
---

Add `WebviewBuilderExtWindows::with_additionl_browser_args` method to pass additional browser args to Webview2 On Windows. By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` so if you use this method, you also need to disable these components by yourself if you want.
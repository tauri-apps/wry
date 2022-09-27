---
"wry": "patch"
---

Add `WebviewBuilderExtWindows::disable_additionl_browser_args` method to prevent wry from passing additional browser args to Webview2 On Windows. By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection` so if you use this method, you also need to add disable these components by yourself if you want.
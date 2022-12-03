---
"wry": "minor"
---

Change the type of `WebViewBuilderExtWindows::with_additional_browser_args` argument from `AsRef<str>` to `Into<String>` to reduce extra allocation.

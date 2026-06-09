---
"wry": patch
---

On macOS 11, fix use-after-free crash in custom protocol `stop_task` during WKWebView dealloc by using raw pointers instead of objc2 references and enforcing correct drop ordering in the async response handler.

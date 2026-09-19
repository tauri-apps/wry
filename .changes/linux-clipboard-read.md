---
"wry": minor
---

Add `read_clipboard_text()` to `WebViewExtUnix` on Linux/BSD. Reads clipboard text asynchronously via `GdkClipboard::read_text_async` and invokes the supplied callback on the GLib main context with `Option<String>`. Avoids the blocking spin-loop pattern and does not require `with_clipboard(true)`.

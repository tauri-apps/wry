---
"wry": minor
---

Add `write_clipboard_text()` to `WebViewExtUnix` on Linux/BSD. Writes text to the system clipboard synchronously via `GdkClipboard` without requiring `with_clipboard(true)` or a JavaScript round-trip.

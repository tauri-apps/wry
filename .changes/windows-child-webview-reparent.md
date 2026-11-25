---
"wry": patch
---

On Windows, update the stored parent window when a child webview is reparented so `bounds()` and `focus_parent()` use the new parent.

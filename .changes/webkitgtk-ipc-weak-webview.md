---
"wry": patch
---

On Linux, fix a destroyed webview and its WebKit web process never being freed: the IPC handler now holds the webview weakly instead of strongly, which made a cycle through the webview's own `UserContentManager`.

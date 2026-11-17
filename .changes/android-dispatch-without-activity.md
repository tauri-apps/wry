---
"wry": patch
---

On Android, fix a panic in `dispatch` when there is no activity, for example after the last one was destroyed. The closure now runs with a null activity and a null webview.

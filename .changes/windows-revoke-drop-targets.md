---
wry: patch
---

On Windows, revoke the drop targets of a webview with a drag drop handler before its windows are destroyed. They were never revoked, so every closed webview leaked them and its drag drop handler. The drop targets now also hold the handler weakly, so it is released with the webview even when a target cannot be revoked.

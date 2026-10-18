---
"wry": patch:bug
---

On Android, take the IPC request URL from the sending frame's origin, so a page's first IPC call no longer arrives as `about:blank`.

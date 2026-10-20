---
"wry": patch:bug
---

On Android WebViews supporting `WEB_MESSAGE_LISTENER`, use the sending frame's origin to avoid misattributing early IPC messages to `about:blank`.

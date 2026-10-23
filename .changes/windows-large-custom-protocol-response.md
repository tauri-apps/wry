---
"wry": patch
---

Fix a panic on Windows when a custom protocol handler responds with a body larger than 4 GiB. The webview now receives an error response instead.

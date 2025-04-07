---
wry: patch
---

Moved protocol handler functions to a thread local instead of storing them as ivars to prevent a race condition between webview close and custom protocol handling.

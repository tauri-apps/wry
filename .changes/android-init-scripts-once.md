---
wry: patch
---

On Android, run initialization scripts once on URLs that are not intercepted by a custom protocol when the WebView supports `DOCUMENT_START_SCRIPT`, instead of a second time from `onPageStarted`.

---
wry: patch
---

On Android, stop the WebView from applying the `Range` header a second time to a custom protocol `206` response, and serve ranges past 2 GiB.

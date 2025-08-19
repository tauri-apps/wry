---
wry: patch
---

On Linux, removed a workaround which forced inital requests for multiple webviews to be handled sequentially.
The workaround was intended to fix a concurrency bug with loading multiple URIs at the same time on WebKitGTK.
But it prevented parallelization and could cause a deadlock in certain situations.
It is no longer needed with newer WebKitGTK versions.

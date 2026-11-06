---
wry: patch
---

On macOS and iOS, download responses sent with `Content-Disposition: attachment` even when WebKit can display their MIME type (PDF, images, text). They were committed as an empty page before, so neither the content nor the download handlers appeared. This matches the Windows and Linux backends.

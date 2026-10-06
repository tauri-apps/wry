---
"wry": patch
---

Avoid Android file-chooser crashes when a video capture or file picker returns a successful result with no Intent, or a picker Intent has no MIME type. Missing video URIs cancel the selection instead of leaving the WebView callback pending.

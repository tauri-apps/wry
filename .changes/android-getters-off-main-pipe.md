---
"wry": patch:bug
---

On Android, `WebView::url`, `can_go_back`, `can_go_forward`, `cookies_for_url` and `webview_version` read on the calling thread instead of waiting on the UI thread, so calling them while the UI thread waits on the caller no longer freezes for 10 s and aborts.

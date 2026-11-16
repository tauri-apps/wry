---
wry: minor
---

On Android, add `WryActivity#onWebViewHistoryChanged`, called from `RustWebViewClient#doUpdateVisitedHistory` whenever the webview's history changes (a navigation, a `pushState`, back or forward), so the activity can follow `canGoBack()`, for example to enable its back callback only while the webview can go back.

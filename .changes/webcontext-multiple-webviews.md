---
"wry": patch
---

Support having multiple webkit2gtk `WebView`s on a single `WebContext`. Custom protocols on unix platforms should only
be registered a single time, and will otherwise return an error and not finish initializing the webview.
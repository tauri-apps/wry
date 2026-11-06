---
"wry": patch:bug
---

On Android WebViews supporting `WEB_MESSAGE_LISTENER`, take the IPC request URL from the WebView for main-frame messages and from the sender's origin for iframes, so a page's first IPC call no longer arrives as `about:blank`.

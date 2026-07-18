---
"wry": patch
---

On macOS, keep `window.opener` alive for `window.open` popups (fixes OAuth/SSO
postMessage logins) by reusing the opener's `WKWebViewConfiguration` with its
user scripts and script message handlers stripped first, avoiding a
duplicate-handler crash on recent macOS.

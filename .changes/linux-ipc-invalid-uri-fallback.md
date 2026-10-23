---
"wry": patch
---

On Linux/BSD, when the webview's current document URL is not a valid `http::Uri`, the IPC handler falls back to `about:blank` as the request URI and still delivers the message, rather than dropping it. This diverges deliberately from the other backends (see #1772): under gtk4/webkit6 a `load_html` page has a `file://` base URI, which `http::Uri` rejects because of its empty authority, so dropping such requests would disable IPC for every `load_html` page. The fallback is logged under the `tracing` feature.

---
"wry": patch
---

Don't panic in the IPC handler when the webview's current document URL is not a valid `http::Uri` (for example a `file://` URL, or one containing a raw space or non-ASCII characters). Such invalid IPC requests are now logged (under the `tracing` feature) and dropped instead of aborting the process, matching the existing behavior of the WKWebView backend.

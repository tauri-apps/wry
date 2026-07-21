---
"wry": patch
---

On Windows, fixed a panic in the WebView2 IPC handler when the posting document's `Source` URL is not a valid `http::Uri` (e.g. `file:`, `data:` or `blob:` documents). The failed `Request` build was unwrapped inside a callback that cannot unwind, aborting the process; the message is now dropped instead.

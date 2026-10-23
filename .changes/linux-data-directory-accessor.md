---
"wry": minor
---

Add `WebViewExtUnix::data_directory() -> Option<PathBuf>` on Linux. Returns the base data
directory used by the underlying WebKitGTK `NetworkSession`, or `None` when using a default
or ephemeral session. This gives callers a way to enumerate or delete persisted data on the
filesystem, mirroring the macOS `fetch_data_store_identifiers` / `remove_data_store` APIs.

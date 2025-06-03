---
wry: minor
---

Added `linux-request-queue` feature flag (enabled by default, which matches previous behavior).
The queue prevents an unknown concurrency bug with loading multiple URIs at the same time on webkit2gtk.
But it can introduce a deadlock situation under certain conditions (https://github.com/tauri-apps/tauri/issues/12589) and it prevents parallelization of request loading.

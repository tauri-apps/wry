---
"wry": minor
---

Add `WebViewBuilderExtUnix::with_data_directory(path)` on Linux. Passing a path creates an
isolated WebKit `NetworkSession` whose persistent data (cache, IndexedDB, cookies, …) lives
under that directory rather than the default system-wide WebKit data directory. This is the
Linux equivalent of `with_profile_name` on Windows and `with_data_store_identifier` on macOS.
The option is silently ignored when an explicit `WebContext` is also supplied — configure
the data directory on the `WebContext` directly in that case.

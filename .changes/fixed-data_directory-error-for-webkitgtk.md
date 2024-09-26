---
"wry": patch
---

On Linux, fixed incorrect path for indexeddb database directory which made apps using `wry@0.24` and `tauri@1` migrating to `wry@>=0.38` and `tauri@2` lose their indexeddb data.

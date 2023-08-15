---
"wry": "patch"
---

Backport `WebViewBuilderExtWindows::with_https_scheme` from `wry@0.32.x` to `wry@0.24.x` to be able to choose between `http` and `https` for custom protocols on Windows. Note that the default behavior for this release is to use `https` unlike `wry@0.32.x` which uses `http` by default.

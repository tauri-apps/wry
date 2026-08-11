---
"wry": patch
---

On Android, cache the asset-loader setting in `RustWebViewClient` so intercepted requests avoid repeated JNI calls into Rust state.

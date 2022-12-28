---
"wry": patch
---

Fix `custom_protocol` example caused an error when instantiating the Wasm module. `WebAssembly.instantiateStreaming` is not supported by WKWebView yet. `WebAssembly.instantiate` should be used instead.

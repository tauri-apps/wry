---
wry: patch
---

On macOS, a webview built on its opener's configuration (`NewWindowResponse::Create` with `WebViewBuilderExtMacos::with_webview_configuration`) shares the opener's `WKUserContentController`, so it no longer adds its own initialization scripts and IPC handler to it: the opener's scripts and `ipc` handler already serve it. This stops every opened window from growing the opener's script set, and stops dropping the new webview from removing the opener's `ipc` handler.

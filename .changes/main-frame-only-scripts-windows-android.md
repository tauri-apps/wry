---
"wry": patch:bug
---

On Windows and Android, initialization scripts registered with `for_main_frame_only` (the default for `WebViewBuilder::with_initialization_script`) no longer run in subframes. Both platforms inject document-start scripts into every frame, so wry now wraps these scripts in an `if (window === window.top) { ... }` guard. On Windows the internal `window.ipc` bridge is guarded the same way, matching Linux and macOS. On Android, when `addDocumentStartJavaScript` is not supported, main-frame-only scripts are no longer inserted into subframe HTML documents served by custom protocols.

---
"wry": patch
---

On Windows, restore keyboard focus into child webviews when the host window is re-activated (Alt+Tab / clicking back to the window). Webviews created via `build_as_child` (including the main webview under Tauri's `unstable` multi-webview feature, which is created as a child) previously never had the focus-restoration subclass attached, so keyboard input was lost until the user clicked inside the content. The handler is now attached for the first webview of a window regardless of `is_child`, and a deferred `MoveFocus` on `WM_ACTIVATE` re-seeds focus when Windows routes activation to the top-level but keyboard focus to the WebView2 child. (#1754)

---
"wry": "patch"
---

**Breaking change**: Refactored the file-drop handling on the webview for better representation of the actual drag and drop operation:

- Renamed `file-drop` cargo feature flag to `drag-drop`.
- Removed `FileDropEvent` enum and replaced with a new `DragDropEvent` enum.
- Renamed `WebViewAttributes::file_drop_handler` field to `WebViewAttributes::drag_drop_handler`.
- Renamed `WebViewAttributes::with_file_drop_handler` method to `WebViewAttributes::with_drag_drop_handler`.

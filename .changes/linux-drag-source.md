---
"wry": minor
---

Add `with_drag_source_handler()` to `WebViewBuilderExtUnix` on Linux/BSD and a new `DragDropEvent::Start` enum variant. The handler receives the pointer `(x, y)` position when the user begins a drag gesture out of the webview and should return the text content to drag, or `None` to cancel. Implemented via GTK4's `GtkDragSource` controller.

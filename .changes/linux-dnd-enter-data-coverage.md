---
"wry": patch
---

Fix `DragDropEvent::Enter` not firing at hover time on Wayland (GTK4/webkit6). The `GtkDropTarget` was created without `preload = true`, so GDK never initiated the `wl_data_device` async transfer until the drop signal — meaning file paths were unavailable in the enter callback and `Enter` was silently skipped. A `notify::value` handler now fires `Enter` with paths as soon as GDK delivers the preloaded payload, matching the synchronous enter-with-paths behaviour of the macOS (`NSDraggingInfo` pasteboard) and Windows (`IDropTarget::DragEnter`) backends. On X11 the existing synchronous path is unchanged. A safety-net fallback in the drop handler preserves the correct event order even if preload data never arrives.

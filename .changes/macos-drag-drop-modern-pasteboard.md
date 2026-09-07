---
"wry": patch
---

On macOS, use the modern `readObjectsForClasses:options:` pasteboard API to collect dragged file paths. This fixes a panic when the drag source publishes only the modern per-item `public.file-url` type (as `NSDraggingItem.initWithPasteboardWriter:` does) instead of the legacy `NSFilenamesPboardType`, and removes the deprecated API as the primary path. The legacy type is kept as a defensive fallback.

---
"wry": patch
---

On macOS, collect dragged file paths via `NSPasteboard.readObjectsForClasses:options:` instead of the deprecated `NSFilenamesPboardType`, fixing a panic in `drag_drop::collect_paths` when the drag source only publishes per-item `public.file-url` types or file reference URLs that AppKit cannot map to `NSFilenamesPboardType`. The legacy type is kept as a fallback.

---
"wry": patch
---

Update Linux documentation to reflect native Wayland support. Top-level module doc, `build()`, and `build_as_child()` now describe the `--features wayland` path alongside X11, replacing the previous "X11 only, use build_gtk for Wayland" wording. `LINUX.md` gains a HiDPI/scaling note for the Wayland embedding path and documents the fractional-scale limitation (requires `gdk4/v4_12`).

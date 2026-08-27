---
"wry": patch
---

Fix three bugs in the Linux Wayland native embedding backend.

`reparent_window()` now correctly handles child-mode WebViews: it finds-or-creates a
`GtkFixed` on the destination window (matching the `new_wayland()` child path) and
re-places the WebView at its tracked position via `GtkFixed::put`. Previously it always
created a `GtkBox`, which left `set_bounds()` unable to find a `GtkFixed` parent and
silently doing nothing after a reparent.

`new()` and `new_as_child()` now return `Error::WaylandNotSupported` — with a message
directing callers to `build_gtk` — when a `RawWindowHandle::Wayland` is passed while the
`wayland` feature is disabled. Previously the generic `Error::UnsupportedWindowHandle`
was returned with no actionable hint.

Dropping a non-child Wayland WebView now clears the host `GtkWindow`'s child widget
(the `GtkBox` wrapper created at construction time), leaving the window clean for reuse.
Child-mode drop is unchanged: the `GtkFixed` container may be shared with other content
and is left in place.

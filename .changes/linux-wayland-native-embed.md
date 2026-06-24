---
"wry": minor
---

On Linux, add native Wayland embedding support via `RawWindowHandle::Wayland`.

`WebViewBuilder::new()` and `WebViewBuilder::new_as_child()` now accept a Wayland window handle when the `wayland` cargo feature is enabled. The embedding strategy finds the `GtkWindow` that owns the given `wl_surface` (by iterating GTK toplevels and comparing raw surface pointers), then attaches the `WebView` as a GTK widget child — using `GtkFixed` for child-mode positioning and `GtkBox` for full-window mode.

`set_bounds()`, `bounds()`, `set_visible()`, and `reparent_window()` all handle the Wayland path: sizing and positioning use `GtkFixed::move_` and `set_size_request` instead of `XMoveResizeWindow`, position is tracked in `WaylandData` for `bounds()` to return, and visibility is toggled via `gtk::Widget::show`/`hide`.

Enable with `--features wayland` (mirrors the existing `x11` feature). The `wayland` feature is not in `default`; both features can be active simultaneously with runtime dispatch on the window handle type.

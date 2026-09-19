---
"wry": patch
---

Remove direct calls to the internal GTK layout API `widget.size_allocate()` from the Linux backend. The X11 GTK window no longer has `size_allocate()` called on it — the X11 configure event drives GTK window sizing automatically. Child WebViews in a `GtkFixed` parent now use `GtkFixed::move_()` and `Widget::set_size_request()`, which are the correct public GTK4 APIs for positioning and sizing within a fixed container.

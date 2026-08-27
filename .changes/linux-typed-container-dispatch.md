---
"wry": patch
---

Replace container type detection via `widget.type_().name()` string comparison with `dynamic_cast_ref::<gtk::Box>()` / `dynamic_cast_ref::<gtk::Fixed>()` in `add_to_container()` and `reparent()`. The old string-based approach is a GTK3-era pattern that does not handle subclasses. The typed downcast correctly matches any subclass of `GtkBox` or `GtkFixed`.

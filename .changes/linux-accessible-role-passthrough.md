---
"wry": patch
---

Set `AccessibleRole::None` on the GTK4 wrapper `GtkBox` widgets that wry creates around WebViews (`create_gtk_window`, `new_wayland`, `add_to_container`). This tells AT-SPI2 to pass accessibility events directly to WebKit's own accessible tree instead of surfacing the generic container node, avoiding duplicate or empty nodes in screen-reader output.

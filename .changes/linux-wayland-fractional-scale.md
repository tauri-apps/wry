---
"wry": patch
---

Add `scale_factor_wayland()` helper that reads the display scale from the GDK surface of the GTK window hosting the embedded WebView. Used in `new_wayland()` at construction time and in `set_bounds()` when the Wayland path is active, so that logical ↔ physical pixel conversions use the surface's actual scale rather than the not-yet-realized WebView widget's scale. Lays the groundwork for fractional scaling via `gdk4::SurfaceExt::scale()` once the `gdk4 v4_12` feature is opted into.

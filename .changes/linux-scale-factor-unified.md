---
"wry": patch
---

Replace the Wayland-only `scale_factor_wayland()` helper with a unified `scale_factor_for_gtk_window()` that works on both X11 (GDK 4.12+) and Wayland. The new helper tries `NativeExt::surface(window).scale()` first (fractional scale, correct on GNOME 45+ at 1.25× / 1.5×) and falls back to the integer `window.scale_factor()`. The `add_to_container` fixed-child path now uses the unified helper for both X11 and Wayland, replacing a separate integer-only fallback for X11.

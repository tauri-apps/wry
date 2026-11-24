---
"wry": minor
---

**Breaking change:** On Linux, updated `gtk` and `gdkx11` to 0.19, `webkit2gtk` and `webkit2gtk-sys` to 3, `javascriptcore-rs` to 2 and `soup3` to 0.9. These types are part of wry's Linux API, so `tao` and any other GTK crates you use must be updated to the matching versions.

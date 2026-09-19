---
"wry": patch
---

Remove `set_enable_preedit(false)` workaround on Linux GTK4/webkit6. The underlying WebKitGTK bug that caused the Fcitx IME popup to anchor at (0,0) was fixed upstream in WebKitGTK 2.44 (commit f69bdc37). Keeping the call unnecessarily broke inline preedit composition for CJK users and caused Fcitx/Fcitx5 to silently drop the first character of subsequent words (Mozilla bug #1742039). Fcitx5 and IBus inline composition now work without restriction on webkitgtk-6.0.

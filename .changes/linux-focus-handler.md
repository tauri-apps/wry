---
"wry": minor
---

Add `with_focus_handler()` to `WebViewBuilderExtUnix` on Linux/BSD. The handler receives `true` when the webview gains keyboard focus and `false` when it loses it, implemented via GTK4's `GtkEventControllerFocus`. Useful for dimming UI overlays, pausing animations, or synchronising state when the webview is not the active input target.

---
"wry": minor
---

Add `with_keyboard_handler()` to `WebViewBuilderExtUnix` on Linux/BSD. The handler receives `(keyval: u32, keycode: u32, modifiers: gdk::ModifierType)` for every key-press before WebKit processes it. Returning `true` consumes the event (WebKit does not see it); returning `false` lets it propagate normally. Implemented via GTK4's `GtkEventControllerKey`.

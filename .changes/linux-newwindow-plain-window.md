---
"wry": patch
---

Fix `NewWindowResponse::Allow` panicking when the root widget is a plain `gtk::Window` instead of a `gtk::ApplicationWindow`. The handler now falls back to creating a plain window in that case, and returns `None` only if the root widget cannot be cast to any window type.

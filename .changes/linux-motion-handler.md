---
"wry": minor
---

Add pointer motion and enter/leave handlers on Linux/BSD via
`WebViewBuilderExtUnix`:

- `with_motion_handler(impl Fn(f64, f64))` — fires on every pointer-move
  over the webview, with `(x, y)` in widget-local logical pixels.
- `with_pointer_enter_handler(impl Fn(f64, f64))` — fires when the pointer
  enters the webview widget area.
- `with_pointer_leave_handler(impl Fn())` — fires when the pointer leaves.

Implemented via `GtkEventControllerMotion` (GTK 4).

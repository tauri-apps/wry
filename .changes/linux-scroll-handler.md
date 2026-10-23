---
"wry": minor
---

Add `WebViewBuilderExtUnix::with_scroll_handler` on Linux/BSD.

The handler receives `(delta_x, delta_y)` in scroll units (positive =
right/down) and returns `bool`: `true` consumes (suppresses) the event
before WebKit sees it; `false` lets it propagate normally.

Implemented via `GtkEventControllerScroll` with the `BOTH_AXES` flag,
following the same return-to-consume convention as `drag_drop_handler`.

---
"wry": patch
---

Replace the GLib spin-loop in all four cookie methods (`cookies_for_url`, `cookies`,
`set_cookie`, `delete_cookie`) on Linux with `glib::MainContext::block_on` and the
webkit6 `_future()` async variants (`cookies_future`, `all_cookies_future`,
`add_cookie_future`, `delete_cookie_future`).

The previous implementation manually called `glib::MainContext::default().iteration(true)`
in a loop until an `std::sync::mpsc` channel received the callback result. This was fragile:
it had no cancellation path if the webview was torn down mid-wait, and the manual iteration
bypassed GLib's nested-loop acquire semantics.

`block_on` creates a proper nested `GMainLoop` on the current main context — the same
mechanism GTK uses for modal dialogs — and runs it until the future resolves. Other GLib
sources (UI events, IPC, timers) continue to be dispatched while waiting.

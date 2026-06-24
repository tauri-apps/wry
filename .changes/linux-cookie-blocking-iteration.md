---
"wry": patch
---

Replace the busy-spin GLib main-loop pump in the four synchronous cookie methods (`cookies_for_url`, `cookies`, `set_cookie`, `delete_cookie`) with a blocking iteration. Changing `glib::MainContext::default().iteration(false)` to `iteration(true)` eliminates the full-CPU spin while waiting for the WebKit cookie-manager callback to fire, without changing the synchronous API surface.

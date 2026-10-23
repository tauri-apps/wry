---
"wry": minor
---

Add `WebViewExtUnix::write_primary_clipboard_text` and
`WebViewExtUnix::read_primary_clipboard_text` on Linux/BSD.

Exposes the X11 primary selection (middle-click clipboard) separately from
the regular clipboard already available via `write_clipboard_text` /
`read_clipboard_text`. On Wayland compositors, write access is restricted
by the compositor security model and the read callback will typically
receive `None`.

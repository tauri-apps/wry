---
"wry": patch
---

Improve rustdoc for builder methods that are silently ignored on Linux (and other platforms):

- `with_hotkeys_zoom` — clarify that it is a no-op on macOS/Linux/Android/iOS because
  WebKitGTK6 has no per-view zoom-key toggle.
- `with_accept_first_mouse` — document that it is a macOS-only concept and a no-op everywhere
  else.
- `with_background_throttling` — document that it is a no-op on Linux/Windows/Android (macOS
  14+ / iOS 17+ feature only).
- `with_general_autofill_enabled` — document that it is a Windows-only browser feature and a
  no-op on all other platforms.
- `with_custom_protocol` and `with_asynchronous_custom_protocol` — add a Linux-specific note
  that request bodies require the `linux-body` feature flag (enabled by default since wry 0.x).

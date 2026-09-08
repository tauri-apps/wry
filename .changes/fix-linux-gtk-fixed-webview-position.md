---
"wry": patch
---

On Linux, fix positioned child webviews in a `gtk::Fixed` losing their position after a layout pass. `WebView::set_bounds` now positions the webview via `gtk::Fixed::move_` (plus `set_size_request`) rather than a bare `size_allocate`, which the `gtk::Fixed` resets to the child's stored put-coordinate on the next size-allocate. This makes positioned multi-webview layouts created with `WebViewBuilderExtUnix::new_gtk` / `build_gtk` over a `gtk::Fixed` hold their bounds across resizes.

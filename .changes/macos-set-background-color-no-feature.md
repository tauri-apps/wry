---
"wry": patch
---

Fix `WebView::set_background_color` being a no-op on macOS.

The runtime `set_background_color` method on macOS previously gated its private
`drawsBackground` KVC call behind the `transparent` feature flag, so it had no effect
unless that flag was enabled. Since the `transparent` feature was removed, the call is
now always made, alongside `setUnderPageBackgroundColor` — a public API available since
macOS 12.

After this change, `set_background_color` on macOS 12+ always replaces the default white
background and updates the overscroll (rubber-band) area color and the background visible
behind page content.

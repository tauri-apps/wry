---
"wry": patch
---

Fix `WebView::set_background_color` being a no-op on macOS without the `transparent` feature.

The runtime `set_background_color` method on macOS previously required the `transparent`
feature flag to have any effect. The flag gates a private `drawsBackground` KVC call that
fully replaces the webview background, but `setUnderPageBackgroundColor` — a public API
available since macOS 12 — does not need it.

After this change, `set_background_color` on macOS 12+ always updates the overscroll
(rubber-band) area color and the background visible behind page content, regardless of
whether the `transparent` feature is enabled. Full background replacement (replacing the
default white with a custom color) still requires the `transparent` feature.

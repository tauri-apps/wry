---
"wry": minor
---

Add `WebView::reparent(window: &impl HasWindowHandle) -> Result<()>` to the core `WebView`
type, providing a unified cross-platform API for moving a webview to a new parent window.
Previously, reparenting required platform-specific extension traits with incompatible
signatures (`WebViewExtWindows::reparent(isize)`, `WebViewExtMacOS::reparent(*mut NSWindow)`,
`WebViewExtUnix::reparent(&W)`). The new method accepts any type implementing
`HasWindowHandle`, mirroring `WebViewBuilder::build`. On Linux it applies to X11-embedded
webviews; GTK-mode webviews created via `build_gtk` continue to use
`WebViewExtUnix::reparent`.

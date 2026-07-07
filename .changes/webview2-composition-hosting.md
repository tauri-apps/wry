---
"wry": minor
---

On Windows, add DirectComposition (visual) hosting for WebView2: `WebViewBuilderExtWindows::with_composition_visual_target` creates the webview through `CreateCoreWebView2CompositionController` targeting a caller-supplied `IDCompositionVisual`, with host-window input forwarding (mouse, touch/pen, cursor, focus, bounds). `register_composition_visual_target` provides the same for embedders that construct the builder internally.

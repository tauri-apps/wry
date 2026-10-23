---
"wry": major
---

Add `WebViewHandle::from_webkit_webview(webkit6::WebView) -> WebViewHandle` on Linux/BSD.

The `WebViewHandleExtUnix` trait (introduced alongside the opaque `WebViewHandle` type)
exposes the underlying platform type through two methods:

- `as_webkit_webview(&self) -> &webkit6::WebView` — borrow the inner webview
- `into_webkit_webview(self) -> webkit6::WebView` — consume the handle and return it

The new constructor allows embedders to wrap an externally-created `webkit6::WebView` in
a `WebViewHandle`. This is needed when responding to `with_new_window_req_handler` with
`NewWindowResponse::Create { webview }` or calling `with_related_view` with a webview
the embedder constructed itself rather than one obtained from wry.

The naming follows the version-agnostic convention used on other platforms
(`as_core_webview2` on Windows, `as_wk_webview` on macOS), so a future webkit version
bump will not require a breaking rename.

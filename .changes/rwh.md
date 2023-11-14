---
"wry": minor
---

Refactor new method to take raw window handle instead. Following are APIs got affected:

- `application` module is removed, and `webivew` module is moved to root module.
- `WebViewBuilder::new`, `WebView::new` now take `RawWindowHandle` instead.
- Add `WebViewBuilder::new_as_child`, `WebView::new_as_child` to crate a webview as a child inside a parent window.
- `Webview::inner_size` is removed.
- Add `WebViewBuilderExtUnix` trait to extend `WebViewBuilder` on Unix platforms.
- Add `new_gtk` functions to `WebViewBuilderExtUnix` and `WebviewExtUnix`.
- [raw-window-handle](https://docs.rs/raw-window-handle/latest/raw_window_handle/) crate is re-exported as `wry::raw_window_handle`.

This also means that we removed `tao` as a dependency completely which required some changes to the public APIs and to the Android backend:

- Webview attributes `ipc_handler`, `file_drop_handler`, `document_change_handler` don't take the `Window` as first parameter anymore.
  Users should use closure to capture the types they want to use.
- Position field in `FileDrop` event is now a tuple of `(x, y)` physical position instead of `PhysicalPosition`. Users need to handle scale factor
- We exposed the `android_setup` function that needs to be called once to setup necessary logic.
- Previously the `android_binding!` had internal call to `tao::android_binding` but now that `tao` has been removed,
  the macro signature has changed and you now need to call `tao::android_binding` yourself, checkout the crate documentation for more information.

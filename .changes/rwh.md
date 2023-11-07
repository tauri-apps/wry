---
"wry": minor
---

Refactor new method to take raw window handle instead. Following are APIs got affected:
  - `application` module is removed, and `webivew` module is moved to root module.
  - `WebviewBuilder::new`, `Webview::new` now take `RawWindowHandle` instead.
  - Attributes `ipc_handler`, `file_drop_handler`, `document_change_handler` don't have window parameter anymore.
  Users should use closure to capture the types they want to use.
  - Position field in `FileDrop` event is now `Position` instead of `PhysicalPosition`. Users need to handle scale factor
  depend on the situation they have.
  - `Webview::inner_size` is removed.

This also means that we removed `tao` as a dependency completely which required some changes to the Android backend:
  - We exposed the `android_setup` function that needs to be called once to setup necessary logic.
  - Previously the `android_binding!` had internal call to `tao::android_binding` but now that `tao` has been removed,sa
    the macro signature has changed and you now need to call `tao::android_binding` yourself, checkout the crate documentation for more information.  
---
"wry": "minor"
---

This release contains quite the breaking changes, because even though wry@0.44, ignored duplicate custom protocols, On Linux when using a shared web context, the custom protocol handler can only be registered once so we are bringing the duplicate custom protocols on Linux again, Windows and macOS are not affected. If using a shared web context, make sure to register a protocol only once on Linux (other platforms should be registed multiple times), use `WebContext::is_custom_protocol_registered` with `#[cfg(target_os = "linux")]`.

We also noticed that it is hard to know which webview made a request to the custom protocol so we added a method to attach an ID to a webview, and changed relevant custom protocol APIs to take a new argument that passes the specified id back to protocol handler.

We also made a few changes to the builder, specifically `WebViewBuilder::new` and `WebViewBuilder::build` methods to make them more ergonomic to work with.

- Added `Error::DuplicateCustomProtocol` enum variant.
- Added `Error::ContextDuplicateCustomProtocol` enum variant.
- On Linux, return an error in `WebViewBuilder::build` if registering a custom protocol multiple times.
- Added `WebContext::is_custom_protocol_registered` to check if a protocol has been regsterd for this web context.
- Added `WebViewId` alias type.
- **Breaking** Changed `WebViewAttributes` to have a lifetime parameter.
- Added `WebViewAttributes.id` field to specify an id for the webview.
- Added `WebViewBuilder::with_id` method to specify an id for the webview.
- Added `WebViewAttributes.context` field to specify a shared context for the webview.
- **Breaking** Changed `WebViewAttributes.custom_protocols` field,`WebViewBuilder::with_custom_protocol` method and `WebViewBuilder::with_asynchronous_custom_protocol` method handler function to take `WebViewId` as the first argument to check which webview made the request to the protocol.
- **Breaking** Changed `WebViewBuilder::with_web_context` to be a static method to create a builder with a webcontext, instead of it being a setter method. It is now an alternative to `WebviewBuilder::new`
- Added `WebViewBuilder::with_attributes` to create a webview builder with provided attributes.
- **Breaking** Changed `WebViewBuilder::new` to take no arguments.
- **Breaking** Changed `WebViewBuilder::build` method to take a reference to a window to create the webview in it.
- **Breaking** Removed `WebViewBuilder::new_as_child`.
- Added `WebViewBuilder::build_as_child` method, which takes a reference to a window to create the webview in it.
- **Breaking** Removed `WebViewBuilderExtUnix::new_gtk`.
- Added `WebViewBuilderExtUnix::build_gtk`.

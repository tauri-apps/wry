---
"wry": minor
---

On Windows, add `WebViewBuilderExtWindows::with_controller` to build a webview from a WebView2 controller the application has created itself (with the environment it came from, given with `with_environment`). An application that must not block its event loop can then create the environment and the controller with WebView2's asynchronous calls and build the webview in the controller's completion handler; the build creates neither and does not wait for them. See the `windows_async_creation` example.

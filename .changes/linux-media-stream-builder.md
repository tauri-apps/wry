---
"wry": minor
---

Add `with_enable_media_stream` and `with_enable_encrypted_media` builder methods.

On Linux (WebKitGTK6), camera/microphone access via `getUserMedia` and Encrypted Media
Extensions (EME/DRM) are disabled by default. Previously, enabling them required
platform-specific code that reached through `WebViewExtUnix::webview()` after the
WebView was built. These are now first-class builder options:

```rust
WebViewBuilder::new()
    .with_enable_media_stream(true)    // enables getUserMedia (camera + mic)
    .with_enable_encrypted_media(true) // enables EME / Widevine DRM
    .with_permission_handler(|kind| PermissionResponse::Allow)
    // ...
```

On Windows, macOS, Android and iOS the methods are accepted and ignored — those
platforms enable both features by default.

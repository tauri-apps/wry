---
"wry": minor
---

Add `with_hardware_acceleration_policy` to `WebViewBuilderExtUnix` on Linux.

Exposes WebKitGTK6's hardware acceleration policy setting through the Wry builder,
allowing callers to force software rendering without relying on environment variables:

```rust
use wry::WebViewBuilderExtUnix;
use webkit6::HardwareAccelerationPolicy;

let webview = WebViewBuilder::new()
    .with_hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
    .build_gtk(&vbox)?;
```

`HardwareAccelerationPolicy::Never` is equivalent to setting `WEBKIT_DISABLE_DMABUF_RENDERER=1`
but is scoped to the individual webview instance. This is particularly useful in headless
environments, CI runners, or systems with an EGL/DRM stack that does not support DMA-BUF
GPU compositing, where the default `Always` policy causes a blank or corrupted webview.

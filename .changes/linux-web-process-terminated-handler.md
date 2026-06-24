---
"wry": minor
---

Add `with_on_web_content_process_terminate_handler` to `WebViewBuilderExtUnix` on Linux.

When the WebKit web process crashes or is terminated (e.g. due to exceeding memory limits),
the provided closure is now called. This brings Linux to parity with macOS, which exposes
the same handler via `WebViewBuilderExtDarwin`.

```rust
use wry::WebViewBuilderExtUnix;

let webview = WebViewBuilder::new()
    .with_on_web_content_process_terminate_handler(|| {
        eprintln!("web process terminated — reloading");
    })
    .build_gtk(&vbox)?;
```

Internally this connects WebKitGTK6's `web-process-terminated` signal. The termination
reason (`Crashed` / `ExceededMemoryLimit`) is not forwarded through the closure; callers
that need it can connect the signal directly via `WebViewExtUnix::webview()`.

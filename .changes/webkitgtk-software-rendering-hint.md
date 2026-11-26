---
wry: patch
---

On WebKitGTK, warn once before the first webview is created when the environment indicates software GL rendering (`LIBGL_ALWAYS_SOFTWARE`, `GALLIUM_DRIVER`) and neither `WEBKIT_DISABLE_DMABUF_RENDERER` nor `WEBKIT_DISABLE_COMPOSITING_MODE` is set, since WebKitGTK renders a blank window in that case without reporting anything. Log-only; no environment variables are modified.

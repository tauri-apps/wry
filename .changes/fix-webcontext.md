---
"wry": patch
---

On macOS and iOS, remove webcontext implementation since we don't actually use it. This also fix segfault if users drop webcontext early.

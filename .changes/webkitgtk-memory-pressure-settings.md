---
"wry": minor
---

On Linux, add `WebContextExtUnix::new_with_memory_pressure_settings` and `MemoryPressureSettings` to create a `WebContext` with custom WebKitGTK memory-pressure settings (memory limit, conservative, strict and kill thresholds, poll interval), which WebKitGTK accepts only when the context is constructed.

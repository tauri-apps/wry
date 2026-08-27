---
"wry": minor
---

Add a Wry-owned `HardwareAccelerationPolicy` enum (`Always` / `Never`). `WebViewBuilderExtUnix::with_hardware_acceleration_policy` accepts `wry::HardwareAccelerationPolicy` directly, so callers do not need to add `webkit6` as a direct dependency just to set this option.

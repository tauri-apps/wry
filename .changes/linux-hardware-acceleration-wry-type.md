---
"wry": minor
---

Add a Wry-owned `HardwareAccelerationPolicy` enum (`Always` / `Never`) so callers no longer need to add `webkit6` as a direct dependency just to set the hardware acceleration policy. `WebViewBuilderExtUnix::with_hardware_acceleration_policy` now accepts `wry::HardwareAccelerationPolicy` instead of `webkit6::HardwareAccelerationPolicy`.

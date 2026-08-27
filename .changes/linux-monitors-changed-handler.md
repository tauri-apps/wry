---
"wry": minor
---

Add `WebViewBuilderExtUnix::with_monitors_changed_handler` and the
`MonitorInfo` struct on Linux/BSD.

The handler fires whenever the monitor configuration changes (monitor
connected, disconnected, or reconfigured) and receives a `Vec<MonitorInfo>`
snapshot of the current monitor list. `MonitorInfo` exposes geometry
(logical pixels), scale factor, and model name.

Implemented via `GdkDisplay::monitors()` → `GListModel::connect_items_changed`.

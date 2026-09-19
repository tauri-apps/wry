---
"wry": minor
---

Expand the macOS `permission_handler` to cover Geolocation and Sensors in addition to
Camera and Microphone. Geolocation is routed via the `WKUIDelegate`
`requestGeolocationPermissionForOrigin:initiatedByFrame:decisionHandler:` callback (macOS 12+).
Sensors (device orientation and motion) are routed via
`requestDeviceOrientationAndMotionPermissionForOrigin:initiatedByFrame:decisionHandler:`.
Both use `WKPermissionDecision` — Grant / Deny / Prompt — consistent with the existing
Camera/Microphone handling. Notifications, ClipboardRead, and PointerLock have no
`WKUIDelegate` hook on macOS and remain unroutable through the permission handler.

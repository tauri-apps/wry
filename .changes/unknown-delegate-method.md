---
wry: patch
---

On macOS in debug mode, don't register `requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:` delegate method on macOS 11 or older to prevent a debug_assertion startup panic.

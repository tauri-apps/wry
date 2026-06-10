---
"wry": patch
---

Fix crash on macOS when `NSOpenPanel::openPanel()` returns nil in rare edge cases (e.g. code-signature mismatch after in-place update, system wake transient). Use raw `msg_send` + `Retained::retain` to detect nil and gracefully cancel the file picker.

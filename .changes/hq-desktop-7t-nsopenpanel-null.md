---
"wry": patch
---

Handle a nil NSOpenPanel result in WebKit's macOS file upload delegate by completing the file selection as cancelled instead of panicking.

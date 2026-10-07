---
wry: patch
---

On macOS, cancel a file upload instead of panicking when `NSOpenPanel` can't be created, for example after the app bundle was replaced on disk while the app was running.

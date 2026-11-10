---
wry: patch
---

On Windows, initialization scripts added with `for_main_frame_only: true` no longer run in subframes. WebView2 injects every document-created script into every frame, so each such script now starts with a check that stops it outside the main frame. The main frame runs the script unchanged, so its top-level `let`, `const` and `class` stay global. An embedded third-party frame no longer gets globals meant for the app page, matching macOS and Linux.

---
"wry": patch
---

Update the windows-rs crate to the latest 0.37.0 release and webview2-com to 0.16.0 to match.

This version of windows-rs depends on rustc version 1.61 for some `const` generic support which was just stabilized, so on Windows the MSRV is effectively 1.61 now.
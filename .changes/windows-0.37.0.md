---
"wry": patch
---

Update the `windows` crate to the latest 0.37.0 release and `webview2-com` to 0.16.0 to match.

The `#[implement]` macro in `windows-implement` depends on `const` generic features which were just stabilized in `rustc` version 1.61, so this change also adds a `rust-version` attribute to the manifest setting the MSRV to 1.61.
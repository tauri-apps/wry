---
"wry": patch
---

Update the `windows` crate to the latest 0.37.0 release and `webview2-com` to 0.16.0 to match.

The `#[implement]` macro in `windows-implement` and the `implement` feature in `windows` depend on some `const` generic features which stabilized in `rustc` 1.61. The MSRV on Windows targets is effectively 1.61, but other targets do not require these features.

The `webview2-com` crate specifies `rust-version = "1.61"`, so `wry` will inherit that MSRV and developers on Windows should get a clear error message telling them to update their toolchain when building `wry` or anything that depends on `wry`. Developers targeting other platforms should be able to continue using whatever toolchain they were using before.

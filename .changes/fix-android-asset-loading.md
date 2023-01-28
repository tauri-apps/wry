---
"wry": patch
---

On Android, `wry` can again load assets from the apk's `asset` folder via a custom protocol. This is set by `WebViewBuilder`'s method `with_asset_loader`, which is exclusive to Android (by virtue of existing within `WebViewBuilderExtAndroid`).

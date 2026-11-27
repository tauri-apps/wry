---
"wry": patch
---

On macOS and iOS, only set the private `allowsPictureInPictureMediaPlayback` key on `WKPreferences` when it responds to a setter for that key, so a WebKit without it skips Picture in Picture instead of raising `NSUnknownKeyException` while creating every webview.

---
wry: patch
---

Override `getDefaultVideoPoster()` on Android to return a transparent bitmap instead of null, preventing Chromium from painting its default gray play-button placeholder on `<video>` elements.

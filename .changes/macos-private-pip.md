---
"wry": patch
---

Gate the private macOS Picture in Picture preference behind the new, disabled-by-default `macos-private-pip` feature. Applications relying on this preference must opt in. On iOS, use WKWebViewConfiguration's public default instead of setting a private WKPreferences key.

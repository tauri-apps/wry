---
"wry": "minor"
---

**Breaking change**: Removed internal url parsing which had a few side-effects such as encoded url content, now it is up to the user to pass a valid URL as a string. This also came with a few breaking changes:

- Removed `Url` struct re-export
- Removed `Error::UrlError` variant.
- Changed `WebviewAttributes::url` field type to `String`.
- Changed `WebviewBuilder::with_url` and `WebviewBuilder::with_url_and_headers` return type to `WebviewBuilder` instead of `Result<WebviewBuilder>`.
- Changed `Webview::url` getter to return a `String` instead of `Url`.

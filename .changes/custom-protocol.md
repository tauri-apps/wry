---
"wry": patch
---

The custom protocol now return a `Request` and expect a `Response`.

- This allow us to get the complete request from the Webview. (Method, GET, POST, PUT etc..)
  Read the complete header.

- And allow us to be more flexible in the future without bringing breaking changes.

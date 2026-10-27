---
"wry": patch:bug
---

On Linux, with the `linux-body` feature, read the whole body of custom protocol requests: a short read no longer ends it, which cut multipart bodies with a file (`<input type="file">` uploads) after their first part.

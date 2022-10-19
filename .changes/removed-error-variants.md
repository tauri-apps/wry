---
"wry": "minor"
---

**Breaking change** Removed http error variants from `wry::Error` and replaced with generic `HttpError` variant that can be used to convert `http` crate errors.
---
"wry": minor
---

Enable `linux-body` by default on Linux.

The `linux-body` feature, which enables reading the HTTP request body (e.g. POST data)
in custom protocol handlers, is now part of the default feature set. Previously it was
opt-in, causing `request.body()` to silently return an empty slice on Linux while
Windows and macOS always provided the body.

The gtk4-webkit6 backend requires WebKitGTK 6.x (well above the v2.40 minimum that
`linux-body` needs), so the version constraint is always satisfied on this branch.

Applications that previously opted in explicitly can remove the `linux-body` feature
from their `Cargo.toml` feature list.

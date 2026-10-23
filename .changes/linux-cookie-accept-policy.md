---
"wry": minor
---

Add `WebContext::set_cookie_accept_policy(policy)` on Linux. Accepts a
`webkit6::CookieAcceptPolicy` value (`Always`, `Never`, or `NoThirdParty`) and applies it to
the `CookieManager` of the underlying `NetworkSession`, affecting all webviews that share the
context. This is a no-op on Windows and macOS.

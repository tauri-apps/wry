---
"wry": "patch"
---

Add `WebViewBuilder::with_limit_navigations_to_app_bound_domains` only on iOS.
Function is a no-op if iOS version is less than iOS 14.
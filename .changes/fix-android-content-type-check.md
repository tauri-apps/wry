---
"wry": patch
---

Make the `Content-Type` spec compliant. This fixes the injection of `intialization_scripts` for devServers where the `Content-Type` header includes more information than just `"text/plain"`.
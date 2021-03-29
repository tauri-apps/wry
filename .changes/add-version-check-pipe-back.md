---
"wry": housekeeping
---

Add pipe back to version check for covector config. This prevents the CI failure on publish if it exists already. The issue was patched in covector (and tests in place so it doesn't break in the future).

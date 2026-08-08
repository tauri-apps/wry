---
wry: patch
---

Make the injected `window.ipc` property configurable so websites can declare their own global `ipc` binding without a syntax error while preserving the existing Wry IPC API.

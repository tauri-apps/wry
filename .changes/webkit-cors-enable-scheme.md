---
"wry": patch
---

On Linux, mark custom URI schemes as CORS-enabled (in addition to
secure) when registering them with webkit2gtk. webkit2gtk 2.46+ added a
requirement that the scheme be in the CORS allow-list for the host's
custom-scheme handler to be invoked for top-level navigations; without
it webkit silently bypasses the handler and routes the request through
the default network loader. Symptom for Tauri apps on Ubuntu 26.04 /
Fedora 40+ / Arch rolling: the bundled UI fails to load and webview
shows "Could not connect to localhost: Connection refused" because
`tauri://localhost/` gets interpreted as `http://localhost:80/`.

The new call is a no-op on webkit2gtk ≤ 2.44 so the patch is safe on
Ubuntu 22.04 / 24.04.

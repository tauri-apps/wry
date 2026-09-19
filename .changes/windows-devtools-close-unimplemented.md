---
"wry": patch
---

Fix `close_devtools()` and `is_devtools_open()` being silent no-ops on Windows.

WebView2 has no `CloseDevToolsWindow()` COM API. Previously `close_devtools()` was
an empty function and `is_devtools_open()` always returned `false`, silently differing
from the Linux and macOS backends.

`is_devtools_open()` now tracks state via an `AtomicBool` that is set when
`open_devtools()` is called and cleared when `close_devtools()` is called. This is
accurate for programmatic open/close; it will not reflect the user manually closing
the DevTools window via its own title bar.

`close_devtools()` now attempts to close the DevTools window by enumerating top-level
windows belonging to the WebView2 browser process (via `BrowserProcessId`) and sending
`WM_CLOSE` to any visible `Chrome_WidgetWin_1` windows found there. This is the
Chromium window class used by the DevTools popup. If no window is found the call is
a no-op, but the open state is still cleared.

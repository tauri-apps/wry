---
"wry": patch:bug
---

On Windows, defer moving the focus into the webview when the hosting window receives `WM_SETFOCUS` until the window activation has finished, and skip it if something else took the focus in the meantime. Calling `MoveFocus` synchronously from `WM_SETFOCUS` left WebView2's IME/TSF input in a broken state when switching between two windows of the same app that host webviews from the same browser process: the IME composition window appeared detached at the top-left corner of the screen and the page never received `compositionstart`.

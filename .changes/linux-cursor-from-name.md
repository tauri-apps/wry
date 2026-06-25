---
"wry": minor
---

Add `WebViewExtUnix::set_cursor_from_name` on Linux/BSD.

Overrides the GTK cursor shown over the webview widget using
`gtk::Widget::set_cursor_from_name`. Accepts any named CSS cursor string
(e.g. `"crosshair"`, `"grab"`, `"zoom-in"`) or `None` to restore the
default browser cursor. Operates independently of CSS `cursor:` properties
on the page.

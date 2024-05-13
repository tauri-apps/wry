---
"wry": minor
---

Default the margin when printing on MacOS to 0 so it is closer to the behavior
of when printing on the web. It also add a new function to WebViewExtMacOS
called print_with_options which allows the user to modify the margins that
will be sent down to the AppKit print operation (NSPrintInfo).
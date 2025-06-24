---
wry: patch
---

On Windows on systems running WebView2 v137+ wry now uses a new default background color API which should reduce white flashes. The use of the `RemoveRedirectionBitmap` browser flag (v134+) was removed due to crashes on Insider builds.

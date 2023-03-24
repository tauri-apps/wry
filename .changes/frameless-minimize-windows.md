---
"wry": patch
---

On Windows when a frameless window (decorations set to false) is minimized it will be resized to a 
small resolution. This resize can cause a significant delay when un-minimizing and redrawing the 
content of the webview after the original window dimensions are restored.
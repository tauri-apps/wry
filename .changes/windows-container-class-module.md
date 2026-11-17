---
"wry": patch
---

On Windows, register the WebView container's window class under the module that holds its window procedure instead of the executable. When wry is built into a DLL that its host unloads and loads again, such as a plug-in, the class no longer keeps a procedure in unmapped memory, which crashed the host at the next WebView.

---
"wry": patch
---

Add `WebviewExtWindows::set_memory_usage_level` API to set the [memory usage target level](https://learn.microsoft.com/en-us/dotnet/api/microsoft.web.webview2.core.corewebview2memoryusagetargetlevel) on Windows. Setting 'Low' memory usage target level when an application is going to inactive can significantly reduce the memory consumption. Please read the [guide for WebView2](https://github.com/MicrosoftEdge/WebView2Feedback/blob/main/specs/MemoryUsageTargetLevel.md) for more details.

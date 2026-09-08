---
"wry": patch
---

On macOS, a Tauri v2 app can lose the WebKit message handler used by Tauri IPC after a popup opened via window.open is handled with tauri::webview::NewWindowResponse::Create and then closed.

After this happens, the opener/main window still has Tauri's JavaScript initialization objects, but later invoke() calls fail because window.webkit.messageHandlers becomes undefined which means it is no longer available.

Fixed by removing the IPC message handler only if it is registered/owned by the popup.

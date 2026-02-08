---
"wry": patch
---

Fix ARM64 WebView2 deadlock by replacing nested message pump with CoWaitForMultipleHandles.

On Windows ARM64 (e.g. Snapdragon X Elite), creating a second WebView2 controller from the
main STA thread would deadlock in MsgWaitForMultipleObjectsEx because the nested
GetMessage/PeekMessage loop prevented COM from re-entering the apartment to deliver the
async completion callback.

This patch replaces the mpsc::channel + wait_with_pump pattern in create_environment(),
create_controller(), and cookies_inner() with CoWaitForMultipleHandles using
COWAIT_DISPATCH_CALLS | COWAIT_DISPATCH_WINDOW_MESSAGES, which is the COM-sanctioned
mechanism for yielding an STA thread while preserving re-entrancy.

Ref: https://github.com/npiesco/wry-arm64-deadlock (minimal reproduction)
---
"wry": minor
---

On Windows, add `WebViewBuilderExtWindows::with_allow_single_sign_on_using_os_primary_account` to let the webview sign in with the signed-in Windows account, the way Microsoft Edge does. This lets sign-in inside the webview pass Microsoft Entra ID Conditional Access policies that require a compliant or managed device.

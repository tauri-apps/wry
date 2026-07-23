---
"wry": patch
---

On Windows, fold the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` environment variable into the args passed through the WebView2 API so it survives WebView2 Runtime 150's elevated-host hardening. This restores WebDriver automation (e.g. msedgedriver's `--remote-debugging-port`) for apps launched by an elevated host such as CI runners.

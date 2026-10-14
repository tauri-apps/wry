---
"wry": minor
---

On Linux, add `WebViewBuilderExtUnix::with_input_method_preedit(bool)` to make the input method preedit configurable. Defaults to `false`, preserving the behavior introduced in wry 0.29.0 (see tauri-apps/tauri#5986). Apps targeting modern webkit2gtk (≥ 2.50) can opt in to inline preedit, which CJK IMEs use to render composition state inside editable elements.

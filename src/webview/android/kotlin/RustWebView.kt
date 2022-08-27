// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{app-domain-reversed}}.{{app-name-snake-case}}

import android.webkit.*
import android.content.Context

class RustWebView(context: Context): WebView(context) {
    init {
        this.settings.javaScriptEnabled = true
        {{class-init}}
    }

    {{class-extension}}
}

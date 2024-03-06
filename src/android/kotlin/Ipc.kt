// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused")

package {{package}}

import android.webkit.*

class Ipc(val webViewClient: RustWebViewClient) {
    @JavascriptInterface
    fun postMessage(message: String?) {
        message?.let {m ->
            this.ipc(webViewClient.currentUrl, m)
        }
    }

    companion object {
        init {
            System.loadLibrary("{{library}}")
        }
    }

    private external fun ipc(url: String, message: String)

    {{class-extension}}
}

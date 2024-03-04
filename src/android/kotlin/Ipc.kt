// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused")

package {{package}}

import android.webkit.*

class Ipc {
    @JavascriptInterface
    fun postMessage(message: String?) {
        message?.let {m ->
            this.ipc(m)
        }
    }

    companion object {
        init {
            System.loadLibrary("{{library}}")
        }
    }

    private external fun ipc(message: String)

    {{class-extension}}
}

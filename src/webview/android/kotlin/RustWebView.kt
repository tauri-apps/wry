// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.annotation.SuppressLint
import android.webkit.*
import android.content.Context
import android.os.Build
import kotlin.collections.Map

class RustWebView(context: Context): WebView(context) {
    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.setGeolocationEnabled(true)
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        settings.javaScriptCanOpenWindowsAutomatically = true
        {{class-init}}
    }

    fun loadUrlMainThread(url: String) {
        post {
          super.loadUrl(url)
        }
    }

    fun loadUrlMainThread(url: String, additionalHttpHeaders: Map<String, String>) {
        post {
          super.loadUrl(url, additionalHttpHeaders)
        }
    }

    fun clearAllBrowsingData() {
        try {
            super.getContext().deleteDatabase("webviewCache.db");
            super.getContext().deleteDatabase("webview.db");
            super.clearCache(true);
            super.clearHistory();
            super.clearFormData();
        } catch (ex: Exception) {
            Logger.error("Unable to create temporary media capture file: " + ex.message)
        }
    }

    fun setAutoPlay(enable: Boolean) {
        post {
          val settings = super.getSettings()
          settings.setMediaPlaybackRequiresUserGesture(!enable)
        }
    }

    fun setUserAgent(ua: String) {
        post {
          val settings = super.getSettings()
          settings.setUserAgentString(ua)
        }
    }

    {{class-extension}}
}

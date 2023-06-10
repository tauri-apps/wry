// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused", "SetJavaScriptEnabled")

package {{package}}

import android.webkit.*
import android.content.Context
import kotlin.collections.Map

class RustWebView(context: Context): WebView(context) {
    private val requestBodyMap: MutableMap<String, String> = mutableMapOf()

    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.setGeolocationEnabled(true)
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        settings.javaScriptCanOpenWindowsAutomatically = true

        val interceptor = Interceptor { id, body ->
          requestBodyMap[id] = body
        }
        addJavascriptInterface(interceptor, "__WRY_INTERCEPTOR__")

        this.webViewClient = RustWebViewClient(context, requestBodyMap)

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

    fun loadHTMLMainThread(html: String) {
        post {
          super.loadData(html, "text/html", null)
        }
    }

    fun clearAllBrowsingData() {
        try {
            super.getContext().deleteDatabase("webviewCache.db")
            super.getContext().deleteDatabase("webview.db")
            super.clearCache(true)
            super.clearHistory()
            super.clearFormData()
        } catch (ex: Exception) {
            Logger.error("Unable to create temporary media capture file: " + ex.message)
        }
    }

    fun setAutoPlay(enable: Boolean) {
        val settings = super.getSettings()
        settings.mediaPlaybackRequiresUserGesture = !enable
    }

    fun setUserAgent(ua: String) {
        val settings = super.getSettings()
        settings.userAgentString = ua
    }

    {{class-extension}}
}

class Interceptor(private val handler: (id: String, body: String) -> Unit) {
    @JavascriptInterface
    fun onRequest(id: String, body: String) {
        handler(id, body)
    }
}

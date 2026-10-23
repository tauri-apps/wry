// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused", "SetJavaScriptEnabled")

package {{package}}

import android.annotation.SuppressLint
import android.webkit.*
import android.content.Context
import android.os.Looper
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import kotlin.collections.Map

@SuppressLint("RestrictedApi")
class RustWebView(context: Context, val initScripts: Array<String>, val id: String): WebView(context) {
    val isDocumentStartScriptEnabled: Boolean

    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.setGeolocationEnabled(true)
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        settings.javaScriptCanOpenWindowsAutomatically = true

        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            isDocumentStartScriptEnabled = true
            for (script in initScripts) {
                WebViewCompat.addDocumentStartJavaScript(this, script, setOf("*"));
            }
        } else {
          isDocumentStartScriptEnabled = false
        }

        {{class-init}}
    }

    fun loadUrlMainThread(url: String) {
        post {
          loadUrl(url)
        }
    }

    fun loadUrlMainThread(url: String, additionalHttpHeaders: Map<String, String>) {
        post {
          loadUrl(url, additionalHttpHeaders)
        }
    }

    override fun loadUrl(url: String) {
        if (!Rust.shouldOverride(id, url)) {
            super.loadUrl(url);
            recordNavigationState()
        }
    }

    override fun loadUrl(url: String, additionalHttpHeaders: Map<String, String>) {
        if (!Rust.shouldOverride(id, url)) {
            super.loadUrl(url, additionalHttpHeaders);
            recordNavigationState()
        }
    }

    override fun goBack() {
        super.goBack()
        recordNavigationState()
    }

    override fun goForward() {
        super.goForward()
        recordNavigationState()
    }

    override fun clearHistory() {
        super.clearHistory()
        recordNavigationState()
    }

    fun loadHTMLMainThread(html: String) {
        post {
          // A null base URL gives about:blank, which IPC accepts as a request URI.
          super.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
          recordNavigationState()
        }
    }

    fun evalScript(id: Int, script: String) {
        post {
            super.evaluateJavascript(script) { result ->
                Rust.onEval(this.id, id, result)
            }
        }
    }

    fun clearAllBrowsingData() {
        try {
            super.getContext().deleteDatabase("webviewCache.db")
            super.getContext().deleteDatabase("webview.db")
            super.clearCache(true)
            clearHistory()
            super.clearFormData()
        } catch (ex: Exception) {
            Logger.error("Unable to create temporary media capture file: " + ex.message)
        }
    }

    // WebView methods only run on the UI thread, so other threads read the state recorded at the
    // last navigation callback. Back and forward steps inside an iframe fire no callback, and other
    // threads see them at the next main-frame navigation.
    @Volatile private var lastUrl: String? = null
    @Volatile private var lastCanGoBack = false
    @Volatile private var lastCanGoForward = false

    fun recordNavigationState() {
        lastUrl = url
        lastCanGoBack = canGoBack()
        lastCanGoForward = canGoForward()
    }

    private fun onUiThread() = Looper.myLooper() == Looper.getMainLooper()

    fun urlFromAnyThread(): String? = if (onUiThread()) url else lastUrl

    fun canGoBackFromAnyThread(): Boolean = if (onUiThread()) canGoBack() else lastCanGoBack

    fun canGoForwardFromAnyThread(): Boolean = if (onUiThread()) canGoForward() else lastCanGoForward

    {{class-extension}}
}

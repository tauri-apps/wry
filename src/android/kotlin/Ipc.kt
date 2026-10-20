// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused")

package {{package}}

import android.net.Uri
import android.webkit.*
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import java.util.concurrent.Executors

class Ipc(val webView: RustWebView, val webViewClient: RustWebViewClient) {
    init {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(webView, "ipc", setOf("*")) { _, message, sourceOrigin, _, _ ->
                // getData throws on ArrayBuffer payloads.
                val body = try { message.data } catch (e: IllegalStateException) { null }
                body?.let { m ->
                    val url = requestUrl(sourceOrigin)
                    // Keep the UI thread available for handlers that wait on the main pipe.
                    dispatcher.execute { Rust.ipc(webView.id, url, m) }
                }
            }
        } else {
            // No sender origin is available, so all frames use the tracked main-frame URL.
            // The URL may remain stale or `about:blank` until `onPageStarted` runs.
            webView.addJavascriptInterface(this, "ipc")
        }
    }

    @JavascriptInterface
    fun postMessage(message: String?) {
        message?.let {m ->
            // we're not using WebView::getUrl() here because it needs to be executed on the main thread
            // and it would slow down the Ipc
            // so instead we track the current URL on the webview client
            Rust.ipc(webView.id, webViewClient.currentUrl, m)
        }
    }

    /** The tracked URL if it shares [origin], else the origin. Opaque origins give about:blank. */
    private fun requestUrl(origin: Uri): String {
        if (origin.host == null) {
            return "about:blank"
        }
        val current = Uri.parse(webViewClient.currentUrl)
        val sameOrigin = current.scheme == origin.scheme && current.host == origin.host && current.port == origin.port
        return (if (sameOrigin) current else origin.buildUpon().path("/").build()).toString()
    }

    companion object {
        private val dispatcher = Executors.newSingleThreadExecutor()
    }

    {{class-extension}}
}

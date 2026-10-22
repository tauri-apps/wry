// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

@file:Suppress("unused")

package {{package}}

import android.net.Uri
import android.webkit.*
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature

class Ipc(val webView: RustWebView, val webViewClient: RustWebViewClient) {
    init {
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(webView, "ipc", setOf("*")) { _, message, sourceOrigin, isMainFrame, _ ->
                // getData throws on ArrayBuffer payloads.
                val body = try { message.data } catch (e: IllegalStateException) { null }
                body?.let { m ->
                    Rust.ipc(webView.id, requestUrl(sourceOrigin, isMainFrame), m)
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

    /** The page URL for main-frame messages from its origin, else the sender's origin. Opaque origins give about:blank. */
    private fun requestUrl(origin: Uri, isMainFrame: Boolean): String {
        if (origin.host == null) {
            return "about:blank"
        }
        // The origin check rejects a pending navigation's URL while the old page still sends.
        val page = if (isMainFrame) webView.url?.let(Uri::parse) else null
        if (page != null && page.scheme == origin.scheme && page.host == origin.host && page.port == origin.port) {
            return page.toString()
        }
        return origin.buildUpon().path("/").build().toString()
    }

    {{class-extension}}
}

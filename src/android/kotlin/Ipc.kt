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
        // webView.url is Chromium's visible URL, which can already name a pending loadUrl target while
        // the previous page still sends, so it is used only when it matches the sender's origin.
        // https://chromium.googlesource.com/chromium/src/+/00e1412eb97d2149f4b63ff16154065f4c0eced2/content/public/browser/web_contents.h#510
        val page = if (isMainFrame) webView.url?.let(Uri::parse) else null
        if (page != null && page.scheme == origin.scheme && page.host == origin.host && page.port == origin.port) {
            return page.toString()
        }
        return origin.buildUpon().path("/").build().toString()
    }

    {{class-extension}}
}

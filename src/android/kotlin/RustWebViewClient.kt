// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.net.Uri
import android.webkit.*
import android.content.Context
import android.graphics.Bitmap
import android.os.Handler
import android.os.Looper
import androidx.webkit.WebViewAssetLoader

class RustWebViewClient(webView: RustWebView, context: Context): WebViewClient() {
    private val interceptedState = mutableMapOf<String, Boolean>()
    var currentUrl: String = "about:blank"
    private var pendingUrlRedirect: String? = null

    private val assetLoader = WebViewAssetLoader.Builder()
        .setDomain(Rust.assetLoaderDomain(webView.id))
        .addPathHandler("/", WebViewAssetLoader.AssetsPathHandler(context))
        .build()

    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest
    ): WebResourceResponse? {
        pendingUrlRedirect?.let {
            Handler(Looper.getMainLooper()).post {
              view.loadUrl(it)
            }
            pendingUrlRedirect = null
            return null
        }

        return if (Rust.withAssetLoader((view as RustWebView).id)) {
            assetLoader.shouldInterceptRequest(request.url)
        } else {
            val response = Rust.handleRequest(view.id, request, view.isDocumentStartScriptEnabled)
            interceptedState[request.url.toString()] = response != null
            return response
        }
    }

    override fun shouldOverrideUrlLoading(
        view: WebView,
        request: WebResourceRequest
    ): Boolean {
        return Rust.shouldOverride((view as RustWebView).id, request.url.toString())
    }

    override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
        currentUrl = url
        if (interceptedState[url] == false) {
            val webView = view as RustWebView
            for (script in webView.initScripts) {
                view.evaluateJavascript(script, null)
            }
        }
        return Rust.onPageLoading((view as RustWebView).id, url)
    }

    override fun onPageFinished(view: WebView, url: String) {
        Rust.onPageLoaded((view as RustWebView).id, url)
    }

    override fun onReceivedError(
        view: WebView,
        request: WebResourceRequest,
        error: WebResourceError
    ) {
        // we get a net::ERR_CONNECTION_REFUSED when an external URL redirects to a custom protocol
        // e.g. oauth flow, because shouldInterceptRequest is not called on redirects
        // so we must force retry here with loadUrl() to get a chance of the custom protocol to kick in
        // 
        // we also get a net::ERR_CONNECTION_REFUSED when a second webview tries to load http://tauri.localhost
        // so we retry the currentUrl regardless. We do not have a timeout yet due to the amount of retries needed to make it work
        // but we might add a timeout in the future
        if (error.errorCode == ERROR_CONNECT) {
            // prevent the default error page from showing
            view.stopLoading()
            // without this initial loadUrl the app is stuck
            view.loadUrl(currentUrl)
            // ensure the URL is actually loaded - for some reason there's a race condition and we need to call loadUrl() again later
            pendingUrlRedirect = currentUrl
        } else {
            super.onReceivedError(view, request, error)
        }
    }

    {{class-extension}}
}

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.webkit.*
import android.content.Context
import android.graphics.Bitmap
import androidx.webkit.WebViewAssetLoader

class RustWebViewClient(context: Context): WebViewClient() {
    private val interceptedState = mutableMapOf<String, Boolean>()
    var currentUrl: String = "about:blank"

    private val assetLoader = WebViewAssetLoader.Builder()
        .setDomain(assetLoaderDomain())
        .addPathHandler("/", WebViewAssetLoader.AssetsPathHandler(context))
        .build()

    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest
    ): WebResourceResponse? {
        return if (withAssetLoader()) {
            assetLoader.shouldInterceptRequest(request.url)
        } else {
            val response = handleRequest(request, (view as RustWebView).isDocumentStartScriptEnabled)
            interceptedState[request.url.toString()] = response != null
            return response
        }
    }

    override fun shouldOverrideUrlLoading(
        view: WebView,
        request: WebResourceRequest
    ): Boolean {
        return shouldOverride(request.url.toString())
    }

    override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
        currentUrl = url
        if (interceptedState[url] == false) {
            val webView = view as RustWebView
            for (script in webView.initScripts) {
                view.evaluateJavascript(script, null)
            }
        }
        return onPageLoading(url)
    }

    override fun onPageFinished(view: WebView, url: String) {
        return onPageLoaded(url)
    }


    companion object {
        init {
            System.loadLibrary("{{library}}")
        }
    }

    private external fun assetLoaderDomain(): String
    private external fun withAssetLoader(): Boolean
    private external fun handleRequest(request: WebResourceRequest, isDocumentStartScriptEnabled: Boolean): WebResourceResponse?
    private external fun shouldOverride(url: String): Boolean
    private external fun onPageLoading(url: String)
    private external fun onPageLoaded(url: String)

    {{class-extension}}
}

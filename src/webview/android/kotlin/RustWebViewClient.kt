// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.webkit.*
import android.content.Context
import androidx.webkit.WebViewAssetLoader

class RustWebViewClient(context: Context): WebViewClient() {
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
            handleRequest(request)
        }
    }

    override fun shouldOverrideUrlLoading(
        view: WebView,
        request: WebResourceRequest
    ): Boolean {
        return shouldOverride(request.url.toString())
    }


    companion object {
        init {
            System.loadLibrary("{{library}}")
        }
    }

    private external fun assetLoaderDomain(): String
    private external fun withAssetLoader(): Boolean
    private external fun handleRequest(request: WebResourceRequest): WebResourceResponse?
    private external fun shouldOverride(url: String): Boolean

    {{class-extension}}
}

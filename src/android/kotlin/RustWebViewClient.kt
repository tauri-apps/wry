// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.net.Uri
import android.webkit.*
import android.content.Context
import android.graphics.Bitmap
import android.os.Build
import android.os.Handler
import android.os.Looper
import androidx.webkit.ServiceWorkerClientCompat
import androidx.webkit.ServiceWorkerControllerCompat
import androidx.webkit.WebViewAssetLoader
import androidx.webkit.WebViewFeature
import java.io.ByteArrayInputStream

class RustWebViewClient(webView: RustWebView, context: Context): WebViewClient() {
    private val interceptedState = mutableMapOf<String, Boolean>()
    var currentUrl: String = "about:blank"
    private var lastInterceptedUrl: Uri? = null
    private var pendingUrlRedirect: String? = null

    private val assetLoader = Rust.assetLoaderDomain(webView.id)?.let { domain ->
        WebViewAssetLoader.Builder()
            .setDomain(domain)
            .addPathHandler("/", WebViewAssetLoader.AssetsPathHandler(context))
            .build()
    }

    private val customProtocolHosts = Rust.customProtocolHosts(webView.id)?.toSet().orEmpty()

    init {
        CustomProtocolHostGuard.blockServiceWorkers(customProtocolHosts)
    }

    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest
    ): WebResourceResponse? {
        pendingUrlRedirect?.let {
            Handler(Looper.getMainLooper()).post {
              view.loadUrl(it)
            }
            pendingUrlRedirect = null
            return forbiddenIfCustomProtocolHost(request.url)
        }

        lastInterceptedUrl = request.url
        return if (assetLoader != null) {
            assetLoader.shouldInterceptRequest(request.url)
        } else {
            val rustWebView = view as RustWebView
            val response = Rust.handleRequest(rustWebView.id, request, rustWebView.isDocumentStartScriptEnabled)
                ?: forbiddenIfCustomProtocolHost(request.url)
            if (response != null) {
                if (response.responseHeaders != null) {
                    response.responseHeaders["Cache-Control"] = "no-store"
                } else {
                    response.responseHeaders = mapOf("Cache-Control" to "no-store")
                }
            }
            interceptedState[request.url.toString()] = response != null
            return response
        }
    }

    override fun shouldOverrideUrlLoading(
        view: WebView,
        request: WebResourceRequest
    ): Boolean {
        // redirects skip shouldInterceptRequest, so reload the host to serve it locally
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N && request.isRedirect && request.isForMainFrame && isCustomProtocolHost(request.url)) {
            view.loadUrl(request.url.toString())
            return true
        }
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
        // custom protocol hosts are real domains, so their loads can fail with any error code
        if ((error.errorCode == ERROR_CONNECT || isCustomProtocolHost(request.url)) && request.isForMainFrame && request.url != lastInterceptedUrl) {
            // prevent the default error page from showing
            view.stopLoading()
            // without this initial loadUrl the app is stuck
            view.loadUrl(request.url.toString())
            // ensure the URL is actually loaded - for some reason there's a race condition and we need to call loadUrl() again later
            pendingUrlRedirect = request.url.toString()
        } else {
            super.onReceivedError(view, request, error)
        }
    }

    private fun isCustomProtocolHost(url: Uri) = CustomProtocolHostGuard.matches(customProtocolHosts, url)

    private fun forbiddenIfCustomProtocolHost(url: Uri) =
        if (isCustomProtocolHost(url)) CustomProtocolHostGuard.forbiddenResponse() else null

    {{class-extension}}
}

private object CustomProtocolHostGuard {
    @Volatile
    private var serviceWorkerHosts = emptySet<String>()
    private var serviceWorkerClientInstalled = false

    fun matches(hosts: Set<String>, url: Uri) =
        hosts.isNotEmpty() && url.host?.lowercase()?.trimEnd('.') in hosts

    fun forbiddenResponse() = WebResourceResponse(
        "text/plain",
        null,
        403,
        "Forbidden",
        hashMapOf("Cache-Control" to "no-store"),
        ByteArrayInputStream(ByteArray(0))
    )

    // the service worker client is process-wide, so it covers every webview's hosts
    @Synchronized
    fun blockServiceWorkers(hosts: Set<String>) {
        if (hosts.isEmpty()) return
        serviceWorkerHosts = serviceWorkerHosts + hosts
        if (serviceWorkerClientInstalled) return
        serviceWorkerClientInstalled = true

        if (!WebViewFeature.isFeatureSupported(WebViewFeature.SERVICE_WORKER_BASIC_USAGE) ||
            !WebViewFeature.isFeatureSupported(WebViewFeature.SERVICE_WORKER_SHOULD_INTERCEPT_REQUEST)) {
            Logger.warn("WebView can't intercept service worker requests to custom protocol hosts")
            return
        }
        ServiceWorkerControllerCompat.getInstance().setServiceWorkerClient(object : ServiceWorkerClientCompat() {
            override fun shouldInterceptRequest(request: WebResourceRequest): WebResourceResponse? =
                if (matches(serviceWorkerHosts, request.url)) forbiddenResponse() else null
        })
    }
}

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
import java.io.InputStream

class RustWebViewClient(webView: RustWebView, context: Context): WebViewClient() {
    private val interceptedState = mutableMapOf<String, Boolean>()
    var currentUrl: String = "about:blank"
    private var lastInterceptedUrl: Uri? = null
    private var pendingUrlRedirect: String? = null
    // the URL wry answered for the newest main-frame request, without its fragment; consumed by the next onPageStarted
    private var servedMainFrameUrl: String? = null
    private var newestMainFrameRequest: WebResourceRequest? = null

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
        if (request.isForMainFrame) {
            synchronized(this) { newestMainFrameRequest = request }
        }
        pendingUrlRedirect?.let {
            Handler(Looper.getMainLooper()).post {
              view.loadUrl(it)
            }
            pendingUrlRedirect = null
            val response = forbiddenIfCustomProtocolHost(request.url)
            recordMainFrameResponse(request, response)
            return response
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
                wrapPartialContentBody(request, response)
            }
            interceptedState[request.url.toString()] = response != null
            recordMainFrameResponse(request, response)
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
        // a POST redirected with 307 or 308 skips shouldOverrideUrlLoading and shouldInterceptRequest,
        // so the real server's response would load on the custom protocol host; reload it locally instead
        // interceptedState can't tell: it keeps every URL ever served, so a repeated POST to one would pass
        val served = synchronized(this) { servedMainFrameUrl.also { servedMainFrameUrl = null } } == url.substringBefore('#')
        if (!served && isCustomProtocolHost(Uri.parse(url))) {
            view.stopLoading()
            view.loadUrl(url)
            return
        }
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

    // shouldInterceptRequest calls run in parallel, so only the newest main-frame request may set servedMainFrameUrl:
    // one that a newer navigation cancelled can finish later, and must not vouch for a redirect to its URL
    private fun recordMainFrameResponse(request: WebResourceRequest, response: WebResourceResponse?) = synchronized(this) {
        if (request === newestMainFrameRequest) {
            servedMainFrameUrl = if (response != null) request.url.toString().substringBefore('#') else null
        }
    }

    private fun isCustomProtocolHost(url: Uri) = CustomProtocolHostGuard.matches(customProtocolHosts, url)

    private fun forbiddenIfCustomProtocolHost(url: Uri) =
        if (isCustomProtocolHost(url)) CustomProtocolHostGuard.forbiddenResponse() else null

    /**
     * Before it reads an intercepted stream, the WebView skips to the first byte of a
     * single range ([embedder_support/android/util/input_stream_reader.cc](https://chromium.googlesource.com/chromium/src/+/main/components/embedder_support/android/util/input_stream_reader.cc)). A 206 body
     * already starts at that byte. This wrapper prevents a second skip.
     *
     * NOTE: Workaround for a Chromium bug: the skip ignores the response status. Reads
     * never return the prefix, so a fix that stops the skip will not break this. Once a
     * fixed WebView version exists, gate this wrapper on it.
     *
     * See https://issues.chromium.org/issues/571186732
     * and https://github.com/tauri-apps/wry/issues/1864
     */
    private fun wrapPartialContentBody(request: WebResourceRequest, response: WebResourceResponse) {
        if (response.statusCode != 206) {
            return
        }

        val range = findHeader(request.requestHeaders, "Range")
        if (range == null || !SINGLE_RANGE_WITH_START.matches(range.trim())) {
            return
        }

        val bodyStart = parseContentRangeStart(findHeader(response.responseHeaders, "Content-Range"))
        if (bodyStart == null) {
            return
        }

        response.data = SkippedPrefixInputStream(bodyStart, response.data)
    }

    private fun findHeader(headers: Map<String, String>, name: String): String? {
        return headers.entries.firstOrNull { it.key.equals(name, ignoreCase = true) }?.value
    }

    /** Returns the start position of a Content-Range value. "bytes 100-199/1000" returns 100. */
    private fun parseContentRangeStart(contentRange: String?): Long? {
        if (contentRange == null || !contentRange.startsWith("bytes ")) {
            return null
        }
        return contentRange.substringAfter("bytes ").substringBefore('-').toLongOrNull()
    }

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

    // the service worker client is process-wide, so it covers every webview's hosts.
    // hosts stay blocked after their webview is destroyed, as another webview may still map them
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

/**
 * The WebView does not skip for a multi-range value. It computes a suffix range
 * ("bytes=-100") from available(), so a prefix would move the start.
 */
private val SINGLE_RANGE_WITH_START = Regex("""bytes=\d+-\d*""")

/**
 * Puts a prefix of [prefixLength] bytes before [body]. Skips use the prefix first.
 * Reads return only bytes from [body].
 */
private class SkippedPrefixInputStream(
    prefixLength: Long,
    private val body: InputStream
) : InputStream() {
    private var prefixRemaining = prefixLength

    override fun available(): Int {
        // Includes the prefix. The WebView checks the range against this value.
        val total = prefixRemaining + body.available()

        // NOTE: Workaround for a Chromium limitation. The WebView reads this value as an
        // Int32. It cannot check a range that ends past Int.MAX_VALUE. When this value is
        // 0, the WebView does not check the range (InputStreamReader::VerifyRequestedRange).
        // It still skips to the start. It sets Content-Length: 0 but sends the full body.
        if (total > Int.MAX_VALUE) {
            return 0
        }
        return total.toInt()
    }

    override fun skip(n: Long): Long {
        if (n <= 0) {
            return 0
        }

        if (prefixRemaining == 0L) {
            return body.skip(n)
        }

        // The WebView reads each skip result as an Int32. It calls skip() again until it
        // reaches the start.
        val skipped = minOf(n, prefixRemaining, Int.MAX_VALUE.toLong())
        prefixRemaining -= skipped
        return skipped
    }

    override fun read(): Int {
        return body.read()
    }

    override fun read(b: ByteArray, off: Int, len: Int): Int {
        return body.read(b, off, len)
    }

    override fun close() {
        body.close()
    }
}

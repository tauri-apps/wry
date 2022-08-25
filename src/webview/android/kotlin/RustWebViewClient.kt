package {{app-domain-reversed}}.{{app-name-snake-case}}

import android.webkit.*

class RustWebViewClient: WebViewClient() {
    override fun shouldInterceptRequest(
        view: WebView,
        request: WebResourceRequest
    ): WebResourceResponse? {
        return handleRequest(request)
    }

    companion object {
        init {
            System.loadLibrary("{{app-name-snake-case}}")
        }
    }

    private external fun handleRequest(request: WebResourceRequest): WebResourceResponse?

    {{class-extension}}
}

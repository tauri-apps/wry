package {{app-domain-reversed}}.{{app-name-snake-case}}

import android.graphics.Bitmap
import android.webkit.*

class RustWebViewClient(initScripts: Array<String>): WebViewClient() {
    private val initializationScripts: Array<String>

    init {
      initializationScripts = initScripts
    }

    override fun onPageStarted(view: WebView?, url: String?, favicon: Bitmap?) {
        for (script in initializationScripts) {
          view?.evaluateJavascript(script, null)
        }
        super.onPageStarted(view, url, favicon)
    }

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
}

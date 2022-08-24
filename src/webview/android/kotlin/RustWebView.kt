package {{app-domain-reversed}}.{{app-name-snake-case}}

import android.webkit.*
import android.content.Context

class RustWebView(context: Context): WebView(context) {
    init {
        this.settings.javaScriptEnabled = true
        {{class-init}}
    }

    {{class-extension}}
}

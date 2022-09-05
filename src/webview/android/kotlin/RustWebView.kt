// Copyright 2020-2022 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{app-domain-reversed}}.{{app-name-snake-case}}

import android.annotation.SuppressLint
import android.webkit.*
import android.content.Context
import android.os.Build

class RustWebView(context: Context): WebView(context) {

    val version: String
        @SuppressLint("WebViewApiAvailability")
        get() {
            val pm = context.packageManager

            // Check getCurrentWebViewPackage() directly if above Android 8
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                return getCurrentWebViewPackage()?.versionName ?: ""
            }

            // Otherwise manually check WebView versions
            try {
                val webViewPackage = "com.android.chrome"
                @Suppress("DEPRECATION")
                val info = pm.getPackageInfo(webViewPackage, 0)
                return info.versionName
            } catch (ex: Exception) {
                Logger.warn("Unable to get package info for 'com.android.chrome'$ex");
            }

            try {
                @Suppress("DEPRECATION")
                val info = pm.getPackageInfo("com.android.webview", 0);
                return info.versionName
            } catch (ex: Exception) {
                Logger.warn("Unable to get package info for 'com.android.webview'$ex");
            }

            // Could not detect any webview, return empty string
            return "";
        }

    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.setGeolocationEnabled(true)
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = false
        settings.javaScriptCanOpenWindowsAutomatically = true
        {{class-init}}
    }

    fun loadUrlMainThread(url: String) {
        post {
          super.loadUrl(url)
        }
    }

    {{class-extension}}
}

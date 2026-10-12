// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

package {{package}}

import android.annotation.SuppressLint
import android.content.ComponentName
import android.content.Intent
import android.content.pm.ActivityInfo
import android.os.Build
import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResult
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner

private const val ACTIVITY_ID_KEY = "__wryActivityId"

private fun Bundle?.activityId(): Int? =
    this?.takeIf { it.containsKey(ACTIVITY_ID_KEY) }?.getInt(ACTIVITY_ID_KEY)

object WryLifecycleObserver : DefaultLifecycleObserver {
    // This only runs once: https://developer.android.com/reference/androidx/lifecycle/ProcessLifecycleOwner
    override fun onCreate(owner: LifecycleOwner) {
        super.onCreate(owner)
        Rust.onFirstActivityCreate()
        Rust.onFirstActivityCreateWry()
    }
}

abstract class WryActivity : AppCompatActivity() {
    private lateinit var mWebView: RustWebView
    private lateinit var permissionLauncher: ActivityResultLauncher<Array<String>>
    private lateinit var activityLauncher: ActivityResultLauncher<Intent>
    private var permissionListener: ((Boolean?) -> Unit)? = null
    private var activityListener: ((ActivityResult?) -> Unit)? = null
    var id: Int = 0
    open val handleBackNavigation: Boolean = true

    open fun onWebViewCreate(webView: WebView) { }

    fun requestPermissions(permissions: Array<String>, listener: (Boolean?) -> Unit) {
        permissionListener = listener
        permissionLauncher.launch(permissions)
    }

    fun launchActivityForResult(intent: Intent, listener: (ActivityResult?) -> Unit) {
        activityListener = listener
        activityLauncher.launch(intent)
    }

    fun setWebView(webView: RustWebView) {
        mWebView = webView

        if (handleBackNavigation) {
            val callback = object : OnBackPressedCallback(true) {
                override fun handleOnBackPressed() {
                    if (this@WryActivity::mWebView.isInitialized) {
                        if (this@WryActivity.mWebView.canGoBack()) {
                            this@WryActivity.mWebView.goBack()
                        } else {
                            this.isEnabled = false
                            this@WryActivity.onBackPressed()
                            this.isEnabled = true
                        }
                    }
                }
            }
            onBackPressedDispatcher.addCallback(this, callback)
        }

        onWebViewCreate(webView)
    }

    val version: String
        @SuppressLint("WebViewApiAvailability", "ObsoleteSdkInt")
        get() {
            // Check getCurrentWebViewPackage() directly if above Android 8
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                return WebView.getCurrentWebViewPackage()?.versionName ?: ""
            }

            // Otherwise manually check WebView versions
            var webViewPackage = "com.google.android.webview"
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
              webViewPackage = "com.android.chrome"
            }
            try {
                @Suppress("DEPRECATION")
                val info = packageManager.getPackageInfo(webViewPackage, 0)
                return info.versionName.toString()
            } catch (ex: Exception) {
                Logger.warn("Unable to get package info for '$webViewPackage'$ex")
            }

            try {
                @Suppress("DEPRECATION")
                val info = packageManager.getPackageInfo("com.android.webview", 0)
                return info.versionName.toString()
            } catch (ex: Exception) {
                Logger.warn("Unable to get package info for 'com.android.webview'$ex")
            }

            // Could not detect any webview, return empty string
            return ""
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        id = savedInstanceState.activityId() ?: intent.extras.activityId() ?: hashCode()

        permissionLauncher = registerForActivityResult(
            ActivityResultContracts.RequestMultiplePermissions(),
        ) { isGranted ->
            permissionListener?.let { listener ->
                val allGranted = isGranted.values.all { it }
                listener(allGranted)
            }
        }
        activityLauncher = registerForActivityResult(
            ActivityResultContracts.StartActivityForResult()
        ) { result ->
            activityListener?.invoke(result)
        }

        Rust.onCreate(this)
        ProcessLifecycleOwner.get().lifecycle.addObserver(WryLifecycleObserver)
    }

    override fun onStart() {
        super.onStart()
        Rust.onStart(this)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        Rust.onWindowFocusChanged(this, hasFocus)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putInt(ACTIVITY_ID_KEY, id)
    }

    override fun onPause() {
        super.onPause()
        Rust.onPause(this)
        if (::mWebView.isInitialized) {
            mWebView.onPause()
        }
    }

    override fun onResume() {
        super.onResume()
        Rust.onResume(this)
        if (::mWebView.isInitialized) {
            mWebView.onResume()
        }
    }

    override fun onStop() {
        super.onStop()
        Rust.onStop(this)
    }

    override fun onDestroy() {
        super.onDestroy()
        Rust.onDestroy(this)
        Rust.onWebviewDestroy(this, if (::mWebView.isInitialized) { mWebView.id } else { "" })
    }

    override fun onLowMemory() {
        super.onLowMemory()
        Rust.onLowMemory()
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        Rust.onNewIntent(intent)
    }

    fun getAppClass(name: String): Class<*> {
        return Class.forName(name)
    }

    // Called by tao through JNI
    fun startActivity(cls: Class<*>): Int {
        return startActivity(cls, false)
    }

    // Called by tao through JNI.
    // Starts an activity of the given class, or of the launcher activity when it is null, and returns its ID.
    // With newTask, the activity gets a task of its own, which the system shows as a separate window
    // (Recents entry, split-screen pane, desktop window); a finished one leaves no stale entry in Recents.
    // Otherwise it is stacked on this activity's task, where activity embedding rules can show the two side by side.
    fun startActivity(cls: Class<*>?, newTask: Boolean): Int {
        val component = if (cls != null) {
            ComponentName(this, cls)
        } else {
            (packageManager.getLaunchIntentForPackage(packageName)
                ?: packageManager.getLeanbackLaunchIntentForPackage(packageName))?.component
                ?: throw IllegalStateException("$packageName has no launcher activity")
        }

        // these launch modes hand the intent to an existing instance instead of creating the activity
        @Suppress("DEPRECATION")
        when (packageManager.getActivityInfo(component, 0).launchMode) {
            ActivityInfo.LAUNCH_SINGLE_TASK, ActivityInfo.LAUNCH_SINGLE_INSTANCE ->
                throw IllegalStateException(
                    "${component.className} can only have one instance (launchMode singleTask or singleInstance); " +
                        "use launchMode standard${if (newTask) " or singleInstancePerTask" else ""} in AndroidManifest.xml"
                )
            ActivityInfo.LAUNCH_SINGLE_INSTANCE_PER_TASK -> if (!newTask) {
                throw IllegalStateException(
                    "${component.className} can only have one instance per task (launchMode singleInstancePerTask); " +
                        "use launchMode standard in AndroidManifest.xml to start it in an existing task"
                )
            }
        }

        val intent = Intent().setComponent(component)
        val id = kotlin.random.Random.nextInt()
        intent.putExtra(ACTIVITY_ID_KEY, id)
        if (newTask) {
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_DOCUMENT or Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
        }
        startActivity(intent)
        return id
    }

    // Called by tao through JNI.
    // With removeTask, the task is removed as well when this activity is its root, so it is gone from Recents too.
    fun closeActivity(removeTask: Boolean) {
        runOnUiThread {
            if (isFinishing || isDestroyed) {
                return@runOnUiThread
            }
            if (removeTask && isTaskRoot) {
                finishAndRemoveTask()
            } else {
                finish()
            }
        }
    }

    {{class-extension}}
}

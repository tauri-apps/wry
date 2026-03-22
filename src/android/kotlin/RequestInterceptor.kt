// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT
// taken from https://github.com/acsbendi/Android-Request-Inspector-WebView
// Copyright 2022 Bendegúz Ács

@file:Suppress("unused")

package {{package}}

import android.util.Log
import android.webkit.JavascriptInterface
import org.json.JSONArray
import org.json.JSONObject
import java.net.URLEncoder

class RequestInterceptor {
    private val interceptedRequests = HashMap<String, RecordedRequest>()

    fun removeInterceptedRequest(id: String): RecordedRequest? {
        return interceptedRequests.remove(id)
    }

    data class RecordedRequest(
        val url: String,
        val body: String,
    )

    @JavascriptInterface
    fun recordFormSubmission(
        url: String,
        formParameterList: String,
        enctype: String?
    ) {
        val formParameterJsonArray = JSONArray(formParameterList)

        val body = when (enctype) {
            "application/x-www-form-urlencoded" -> {
                getUrlEncodedFormBody(formParameterJsonArray)
            }

            "multipart/form-data" -> {
                getMultiPartFormBody(formParameterJsonArray)
            }

            "text/plain" -> {
                getPlainTextFormBody(formParameterJsonArray)
            }

            else -> {
                Log.e("RequestInterceptor", "Incorrect encoding received from JavaScript: $enctype")
                ""
            }
        }

        addRecordedRequest(
            id,
            RecordedRequest(
                url.removeSuffix("/"),
                body
            )
        )
    }

    @JavascriptInterface
    fun recordXhr(id: String, url: String, body: String) {
        addRecordedRequest(
            id,
            RecordedRequest(
                url,
                body
            )
        )
    }

    @JavascriptInterface
    fun recordFetch(id: String, url: String, body: String) {
        addRecordedRequest(
            id,
            RecordedRequest(
                url,
                body
            )
        )
    }

    private fun addRecordedRequest(id: String, recordedRequest: RecordedRequest) {
        interceptedRequests[id] = recordedRequest
    }

    private fun getUrlEncodedFormBody(formParameterJsonArray: JSONArray): String {
        val resultStringBuilder = StringBuilder()
        repeat(formParameterJsonArray.length()) { i ->
            val formParameter = formParameterJsonArray.get(i) as JSONObject
            val name = formParameter.getString("name")
            val value = formParameter.optString("value")
            val checked = formParameter.optBoolean("checked")
            val type = formParameter.optString("type")
            val encodedValue = URLEncoder.encode(value, "UTF-8")

            if (!isExcludedFormParameter(type, checked)) {
                if (i != 0) {
                    resultStringBuilder.append("&")
                }
                resultStringBuilder.append(name)
                resultStringBuilder.append("=")
                resultStringBuilder.append(encodedValue)
            }


        }
        return resultStringBuilder.toString()
    }

    private fun getMultiPartFormBody(formParameterJsonArray: JSONArray): String {
        val resultStringBuilder = StringBuilder()
        repeat(formParameterJsonArray.length()) { i ->
            val formParameter = formParameterJsonArray.get(i) as JSONObject
            val name = formParameter.getString("name")
            val value = formParameter.optString("value")
            val checked = formParameter.optBoolean("checked")
            val type = formParameter.optString("type")

            if (!isExcludedFormParameter(type, checked)) {
                resultStringBuilder.append("--")
                resultStringBuilder.append(MULTIPART_FORM_BOUNDARY)
                resultStringBuilder.append("\n")
                resultStringBuilder.append("Content-Disposition: form-data; name=\"$name\"")
                resultStringBuilder.append("\n\n")
                resultStringBuilder.append(value)
                resultStringBuilder.append("\n")
            }

        }
        resultStringBuilder.append("--")
        resultStringBuilder.append(MULTIPART_FORM_BOUNDARY)
        resultStringBuilder.append("--")
        return resultStringBuilder.toString()
    }

    private fun getPlainTextFormBody(formParameterJsonArray: JSONArray): String {
        val resultStringBuilder = StringBuilder()
        repeat(formParameterJsonArray.length()) { i ->
            val formParameter = formParameterJsonArray.get(i) as JSONObject
            val name = formParameter.getString("name")
            val value = formParameter.optString("value")
            val checked = formParameter.optBoolean("checked")
            val type = formParameter.optString("type")

            if (!isExcludedFormParameter(type, checked)) {
                if (i != 0) {
                    resultStringBuilder.append("\n")
                }
                resultStringBuilder.append(name)
                resultStringBuilder.append("=")
                resultStringBuilder.append(value)
            }

        }
        return resultStringBuilder.toString()
    }

    private fun isExcludedFormParameter(type: String, checked: Boolean): Boolean {
        return (type == "radio" || type == "checkbox") && !checked
    }

    companion object {
        private const val MULTIPART_FORM_BOUNDARY = "----WebKitFormBoundaryU7CgQs9WnqlZYKs6"
        const val INTERFACE_NAME = "RequestInterceptor"
        const val REQUEST_ID_HEADER_NAME = "wry-internal-request-id"
    }
}

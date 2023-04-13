# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class androidx.appcompat.app.AppCompatActivity { }

-keep class {{package}}.* {
  native <methods>;
}

-keep class {{package}}.WryActivity {
  {{package}}.RustWebView m_webview;

  public <init>(...);

  void setM_webview({{package}}.RustWebView);
  {{package}}.RustWebView getM_webview();

  java.lang.Class getAppClass(...);
  java.lang.String getVersion();
}

-keep class {{package}}.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class {{package}}.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void setAutoPlay(...);
}

-keep class {{package}}.RustWebChromeClient,{{package}}.RustWebViewClient {
  public <init>(...);
}
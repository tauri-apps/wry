# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class {{package}}.* {
  native <methods>;
}

-keep class {{package}}.WryActivity {
  public <init>(...);

  void setWebView({{package}}.RustWebView);
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
  void loadHTMLMainThread(...);
  void setAutoPlay(...);
  void setUserAgent(...);
  void evalScript(...);
}

-keep class {{package}}.RustWebChromeClient,{{package}}.RustWebViewClient {
  public <init>(...);
}
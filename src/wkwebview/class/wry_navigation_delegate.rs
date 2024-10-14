// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};

use objc2::{
  declare_class, msg_send_id, mutability::MainThreadOnly, rc::Retained, runtime::NSObject,
  ClassType, DeclaredClass,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
use objc2_web_kit::{
  WKDownload, WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
  WKNavigationResponse, WKNavigationResponsePolicy,
};

#[cfg(target_os = "ios")]
use crate::wkwebview::ios::WKWebView::WKWebView;
#[cfg(target_os = "macos")]
use objc2_web_kit::WKWebView;

use crate::{
  url_from_webview,
  wkwebview::{
    download::{navigation_download_action, navigation_download_response},
    navigation::{
      did_commit_navigation, did_finish_navigation, navigation_policy, navigation_policy_response,
    },
  },
  PageLoadEvent, WryWebView,
};

use super::wry_download_delegate::WryDownloadDelegate;

pub struct WryNavigationDelegateIvars {
  pub pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
  pub has_download_handler: bool,
  pub navigation_policy_function: Box<dyn Fn(String, bool) -> bool>,
  pub download_delegate: Option<Retained<WryDownloadDelegate>>,
  pub on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent)>>,
}

declare_class!(
  pub struct WryNavigationDelegate;

  unsafe impl ClassType for WryNavigationDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryNavigationDelegate";
  }

  impl DeclaredClass for WryNavigationDelegate {
    type Ivars = WryNavigationDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryNavigationDelegate {}

  unsafe impl WKNavigationDelegate for WryNavigationDelegate {
    #[method(webView:decidePolicyForNavigationAction:decisionHandler:)]
    fn navigation_policy(
      &self,
      webview: &WKWebView,
      action: &WKNavigationAction,
      handler: &block2::Block<dyn Fn(WKNavigationActionPolicy)>,
    ) {
      navigation_policy(self, webview, action, handler);
    }

    #[method(webView:decidePolicyForNavigationResponse:decisionHandler:)]
    fn navigation_policy_response(
      &self,
      webview: &WKWebView,
      response: &WKNavigationResponse,
      handler: &block2::Block<dyn Fn(WKNavigationResponsePolicy)>,
    ) {
      navigation_policy_response(self, webview, response, handler);
    }

    #[method(webView:didFinishNavigation:)]
    fn did_finish_navigation(
      &self,
      webview: &WKWebView,
      navigation: &WKNavigation,
    ) {
      did_finish_navigation(self, webview, navigation);
    }

    #[method(webView:didCommitNavigation:)]
    fn did_commit_navigation(
      &self,
      webview: &WKWebView,
      navigation: &WKNavigation,
    ) {
      did_commit_navigation(self, webview, navigation);
    }

    #[method(webView:navigationAction:didBecomeDownload:)]
    fn navigation_download_action(
      &self,
      webview: &WKWebView,
      action: &WKNavigationAction,
      download: &WKDownload,
    ) {
      navigation_download_action(self, webview, action, download);
    }

    #[method(webView:navigationResponse:didBecomeDownload:)]
    fn navigation_download_response(
      &self,
      webview: &WKWebView,
      response: &WKNavigationResponse,
      download: &WKDownload,
    ) {
      navigation_download_response(self, webview, response, download);
    }
  }
);

impl WryNavigationDelegate {
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    webview: Retained<WryWebView>,
    pending_scripts: Arc<Mutex<Option<Vec<String>>>>,
    has_download_handler: bool,
    navigation_handler: Option<Box<dyn Fn(String) -> bool>>,
    new_window_req_handler: Option<Box<dyn Fn(String) -> bool>>,
    download_delegate: Option<Retained<WryDownloadDelegate>>,
    on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
    mtm: MainThreadMarker,
  ) -> Retained<Self> {
    let navigation_policy_function = Box::new(move |url: String, is_main_frame: bool| -> bool {
      if is_main_frame {
        navigation_handler
          .as_ref()
          .map_or(true, |navigation_handler| (navigation_handler)(url))
      } else {
        new_window_req_handler
          .as_ref()
          .map_or(true, |new_window_req_handler| (new_window_req_handler)(url))
      }
    });

    let on_page_load_handler = if let Some(handler) = on_page_load_handler {
      let custom_handler = Box::new(move |event| {
        handler(event, url_from_webview(&webview).unwrap_or_default());
      }) as Box<dyn Fn(PageLoadEvent)>;
      Some(custom_handler)
    } else {
      None
    };

    let delegate = mtm
      .alloc::<WryNavigationDelegate>()
      .set_ivars(WryNavigationDelegateIvars {
        pending_scripts,
        navigation_policy_function,
        has_download_handler,
        download_delegate,
        on_page_load_handler,
      });

    unsafe { msg_send_id![super(delegate), init] }
  }
}

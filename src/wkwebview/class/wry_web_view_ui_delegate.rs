// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "macos")]
use std::ptr::null_mut;

use block2::Block;
use objc2::{
  declare_class, msg_send_id, mutability::MainThreadOnly, rc::Retained, runtime::NSObject,
  ClassType, DeclaredClass,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSModalResponse, NSModalResponseOK, NSOpenPanel};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSURL};

#[cfg(target_os = "macos")]
use objc2_web_kit::WKOpenPanelParameters;
use objc2_web_kit::{
  WKFrameInfo, WKMediaCaptureType, WKPermissionDecision, WKSecurityOrigin, WKUIDelegate,
};

use crate::WryWebView;

pub struct WryWebViewUIDelegateIvars {}

declare_class!(
  pub struct WryWebViewUIDelegate;

  unsafe impl ClassType for WryWebViewUIDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebViewUIDelegate";
  }

  impl DeclaredClass for WryWebViewUIDelegate {
    type Ivars = WryWebViewUIDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryWebViewUIDelegate {}

  unsafe impl WKUIDelegate for WryWebViewUIDelegate {
    #[cfg(target_os = "macos")]
    #[method(webView:runOpenPanelWithParameters:initiatedByFrame:completionHandler:)]
    fn run_file_upload_panel(
      &self,
      _webview: &WryWebView,
      open_panel_params: &WKOpenPanelParameters,
      _frame: &WKFrameInfo,
      handler: &block2::Block<dyn Fn(*const NSArray<NSURL>)>
    ) {
      unsafe {
        if let Some(mtm) = MainThreadMarker::new() {
          let open_panel = NSOpenPanel::openPanel(mtm);
          open_panel.setCanChooseFiles(true);
          let allow_multi = open_panel_params.allowsMultipleSelection();
          open_panel.setAllowsMultipleSelection(allow_multi);
          let allow_dir = open_panel_params.allowsDirectories();
          open_panel.setCanChooseDirectories(allow_dir);
          let ok: NSModalResponse = open_panel.runModal();
          if ok == NSModalResponseOK {
            let url = open_panel.URLs();
            (*handler).call((Retained::as_ptr(&url),));
          } else {
            (*handler).call((null_mut(),));
          }
        }
      }
    }

    #[method(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:)]
    fn request_media_capture_permission(
      &self,
      _webview: &WryWebView,
      _origin: &WKSecurityOrigin,
      _frame: &WKFrameInfo,
      _capture_type: WKMediaCaptureType,
      decision_handler: &Block<dyn Fn(WKPermissionDecision)>
    ) {
      //https://developer.apple.com/documentation/webkit/wkpermissiondecision?language=objc
      (*decision_handler).call((WKPermissionDecision::Grant,));
    }
  }
);

impl WryWebViewUIDelegate {
  pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
    let delegate = mtm
      .alloc::<WryWebViewUIDelegate>()
      .set_ivars(WryWebViewUIDelegateIvars {});
    unsafe { msg_send_id![super(delegate), init] }
  }
}

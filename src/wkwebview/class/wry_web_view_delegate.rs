// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::ffi::CStr;

use http::Request;
use objc2::{
  declare_class, msg_send_id,
  mutability::MainThreadOnly,
  rc::Retained,
  runtime::{NSObject, ProtocolObject},
  ClassType, DeclaredClass,
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSString};
use objc2_web_kit::{WKScriptMessage, WKScriptMessageHandler, WKUserContentController};

pub const IPC_MESSAGE_HANDLER_NAME: &str = "ipc";

pub struct WryWebViewDelegateIvars {
  pub controller: Retained<WKUserContentController>,
  pub ipc_handler: Box<dyn Fn(Request<String>)>,
}

declare_class!(
  pub struct WryWebViewDelegate;

  unsafe impl ClassType for WryWebViewDelegate {
    type Super = NSObject;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebViewDelegate";
  }

  impl DeclaredClass for WryWebViewDelegate {
    type Ivars = WryWebViewDelegateIvars;
  }

  unsafe impl NSObjectProtocol for WryWebViewDelegate {}

  unsafe impl WKScriptMessageHandler for WryWebViewDelegate {
    // Function for ipc handler
    #[method(userContentController:didReceiveScriptMessage:)]
    fn did_receive(
      this: &WryWebViewDelegate,
      _controller: &WKUserContentController,
      msg: &WKScriptMessage,
    ) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();

        let ipc_handler = &this.ivars().ipc_handler;
        let body = msg.body();
        let is_string = Retained::cast::<NSObject>(body.clone()).isKindOfClass(NSString::class());
        if is_string {
          let body = Retained::cast::<NSString>(body);
          let js_utf8 = body.UTF8String();

          let frame_info = msg.frameInfo();
          let request = frame_info.request();
          let url = request.URL().unwrap();
          let absolute_url = url.absoluteString().unwrap();
          let url_utf8 = absolute_url.UTF8String();

          if let (Ok(url), Ok(js)) = (
            CStr::from_ptr(url_utf8).to_str(),
            CStr::from_ptr(js_utf8).to_str(),
          ) {
            ipc_handler(Request::builder().uri(url).body(js.to_string()).unwrap());
            return;
          }
        }

        #[cfg(feature = "tracing")]
        tracing::warn!("WebView received invalid IPC call.");
      }
    }
  }
);

impl WryWebViewDelegate {
  pub fn new(
    controller: Retained<WKUserContentController>,
    ipc_handler: Box<dyn Fn(Request<String>)>,
    mtm: MainThreadMarker,
  ) -> Retained<Self> {
    let delegate = mtm
      .alloc::<WryWebViewDelegate>()
      .set_ivars(WryWebViewDelegateIvars {
        ipc_handler,
        controller,
      });

    let delegate: Retained<Self> = unsafe { msg_send_id![super(delegate), init] };

    let proto_delegate = ProtocolObject::from_ref(delegate.as_ref());
    unsafe {
      // this will increate the retain count of the delegate
      delegate.ivars().controller.addScriptMessageHandler_name(
        proto_delegate,
        &NSString::from_str(IPC_MESSAGE_HANDLER_NAME),
      );
    }

    delegate
  }
}

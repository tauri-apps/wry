// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#[cfg(target_os = "macos")]
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use std::{ffi::CStr, panic::AssertUnwindSafe};

#[cfg(target_os = "macos")]
use crate::WryWebView;
use http::Request;
#[cfg(target_os = "macos")]
use objc2::rc::Weak;
use objc2::{
  DeclaredClass, MainThreadOnly, define_class, msg_send,
  rc::Retained,
  runtime::{NSObject, ProtocolObject},
};
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSString, ns_string};
use objc2_web_kit::{WKScriptMessage, WKScriptMessageHandler, WKUserContentController};

pub const IPC_MESSAGE_HANDLER_NAME: &str = "ipc";

#[cfg(target_os = "macos")]
thread_local! {
  static IPC_DELEGATES: RefCell<HashMap<usize, Weak<WryWebViewDelegate>>> = RefCell::default();
}

#[cfg(target_os = "macos")]
struct IpcRoutes {
  source: Option<usize>,
  callbacks: HashMap<usize, Option<Rc<dyn Fn(Request<String>)>>>,
}

#[cfg(target_os = "macos")]
impl IpcRoutes {
  fn handler(
    &self,
    webview: usize,
    legacy_native_view: bool,
  ) -> Option<Rc<dyn Fn(Request<String>)>> {
    let source_handler = || {
      self
        .source
        .and_then(|source| self.callbacks.get(&source))
        .and_then(Clone::clone)
    };
    match self.callbacks.get(&webview) {
      Some(handler) => handler.clone().or_else(source_handler),
      // Raw native children keep their original source callback while that owner is alive.
      None if legacy_native_view => source_handler(),
      None => None,
    }
  }

  fn retire(&mut self, webview: usize) -> Option<Option<Rc<dyn Fn(Request<String>)>>> {
    if self.source == Some(webview) {
      self.source = None;
    }
    self.callbacks.remove(&webview)
  }
}

pub struct WryWebViewDelegateIvars {
  pub controller: Retained<WKUserContentController>,
  #[cfg(target_os = "ios")]
  pub ipc_handler: Box<dyn Fn(Request<String>)>,
  #[cfg(target_os = "macos")]
  routes: RefCell<IpcRoutes>,
}

define_class!(
  #[unsafe(super(NSObject))]
  #[thread_kind = MainThreadOnly]
  #[ivars = WryWebViewDelegateIvars]
  pub struct WryWebViewDelegate;

  unsafe impl NSObjectProtocol for WryWebViewDelegate {}

  unsafe impl WKScriptMessageHandler for WryWebViewDelegate {
    // Function for ipc handler
    #[unsafe(method(userContentController:didReceiveScriptMessage:))]
    fn did_receive(
      this: &WryWebViewDelegate,
      _controller: &WKUserContentController,
      msg: &WKScriptMessage,
    ) {
      // Safety: objc runtime calls are unsafe
      unsafe {
        #[cfg(feature = "tracing")]
        let _span = tracing::info_span!(parent: None, "wry::ipc::handle").entered();

        #[cfg(target_os = "ios")]
        let ipc_handler = &this.ivars().ipc_handler;
        #[cfg(target_os = "macos")]
        let Some(native_view) = msg.webView() else {
          return;
        };
        #[cfg(target_os = "macos")]
        let legacy_native_view = native_view.downcast_ref::<WryWebView>().is_none();
        #[cfg(target_os = "macos")]
        let Some(ipc_handler) = this
          .ivars()
          .routes
          .borrow()
          .handler(Retained::as_ptr(&native_view) as usize, legacy_native_view)
        else {
          return;
        };
        let body = msg.body();
        if let Ok(body) = body.downcast::<NSString>() {
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
            if let Ok(r) = Request::builder().uri(url).body(js.to_string()) {
              ipc_handler(r);
            } else {
              #[cfg(feature = "tracing")]
              tracing::warn!("WebView received invalid IPC request: {}", js);
            }
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
  #[cfg(target_os = "ios")]
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

    let delegate: Retained<Self> = unsafe { msg_send![super(delegate), init] };

    let proto_delegate = ProtocolObject::from_ref(&*delegate);
    unsafe {
      // this will increase the retain count of the delegate
      let _res = objc2::exception::catch(AssertUnwindSafe(|| {
        delegate
          .ivars()
          .controller
          .addScriptMessageHandler_name(proto_delegate, ns_string!(IPC_MESSAGE_HANDLER_NAME));
      }));
    }

    delegate
  }

  #[cfg(target_os = "macos")]
  pub fn register(
    controller: Retained<WKUserContentController>,
    webview: &WryWebView,
    ipc_handler: Option<Box<dyn Fn(Request<String>)>>,
    mtm: MainThreadMarker,
  ) -> Result<Option<Retained<Self>>, ()> {
    let controller_id = Retained::as_ptr(&controller) as usize;
    let view_id = webview as *const WryWebView as usize;
    let existing = IPC_DELEGATES.with(|delegates| delegates.borrow().get(&controller_id).cloned());
    if let Some(delegate) = existing.and_then(|delegate| delegate.load()) {
      delegate
        .ivars()
        .routes
        .borrow_mut()
        .callbacks
        .insert(view_id, ipc_handler.map(Rc::from));
      return Ok(Some(delegate));
    }
    let Some(ipc_handler) = ipc_handler else {
      return Ok(None);
    };
    let delegate = mtm.alloc::<Self>().set_ivars(WryWebViewDelegateIvars {
      controller,
      routes: RefCell::new(IpcRoutes {
        source: Some(view_id),
        callbacks: HashMap::from([(view_id, Some(Rc::from(ipc_handler)))]),
      }),
    });
    let delegate: Retained<Self> = unsafe { msg_send![super(delegate), init] };
    unsafe {
      objc2::exception::catch(AssertUnwindSafe(|| {
        delegate.ivars().controller.addScriptMessageHandler_name(
          ProtocolObject::from_ref(&*delegate),
          ns_string!(IPC_MESSAGE_HANDLER_NAME),
        );
      }))
      .map_err(|_| ())?;
    }
    IPC_DELEGATES.with(|delegates| {
      delegates
        .borrow_mut()
        .insert(controller_id, Weak::new(&delegate));
    });
    Ok(Some(delegate))
  }

  #[cfg(target_os = "macos")]
  pub fn retire(&self, webview: &WryWebView) {
    let (removed, empty) = {
      let mut routes = self.ivars().routes.borrow_mut();
      let removed = routes.retire(webview as *const WryWebView as usize);
      (removed, routes.callbacks.is_empty())
    };
    if empty {
      let controller_id = Retained::as_ptr(&self.ivars().controller) as usize;
      IPC_DELEGATES.with(|delegates| {
        delegates.borrow_mut().remove(&controller_id);
      });
      unsafe {
        self
          .ivars()
          .controller
          .removeScriptMessageHandlerForName(ns_string!(IPC_MESSAGE_HANDLER_NAME));
      }
    }
    drop(removed);
  }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
  use super::*;
  use std::cell::Cell;

  #[test]
  fn configured_children_keep_their_own_callbacks_after_source_retirement() {
    let source_calls = Rc::new(Cell::new(0));
    let child_calls = Rc::new(Cell::new(0));
    let source = source_calls.clone();
    let child = child_calls.clone();
    let mut routes = IpcRoutes {
      source: Some(1),
      callbacks: HashMap::from([
        (
          1,
          Some(
            Rc::new(move |_: Request<String>| source.set(source.get() + 1))
              as Rc<dyn Fn(Request<String>)>,
          ),
        ),
        (
          2,
          Some(
            Rc::new(move |_: Request<String>| child.set(child.get() + 1))
              as Rc<dyn Fn(Request<String>)>,
          ),
        ),
        (3, None),
      ]),
    };
    routes.handler(2, false).unwrap()(Request::new(String::new()));
    routes.handler(3, false).unwrap()(Request::new(String::new()));
    routes.handler(99, true).unwrap()(Request::new(String::new()));
    assert_eq!((source_calls.get(), child_calls.get()), (2, 1));
    assert!(routes.handler(99, false).is_none());
    routes.retire(1);
    routes.handler(2, false).unwrap()(Request::new(String::new()));
    assert_eq!((source_calls.get(), child_calls.get()), (2, 2));
    assert!(routes.handler(3, false).is_none());
    assert!(routes.handler(99, true).is_none());
    assert!(routes.handler(1, false).is_none());
    routes
      .callbacks
      .insert(1, routes.callbacks.get(&2).unwrap().clone());
    assert!(routes.handler(99, true).is_none());
    assert!(routes.handler(3, false).is_none());
  }
}

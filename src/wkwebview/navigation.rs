use std::{
  ffi::c_void,
  ptr::null_mut,
  sync::{Arc, Mutex},
};

use objc2::{
  rc::Retained,
  runtime::{AnyObject, ClassBuilder, Sel},
};
use objc2_foundation::{NSObject, NSString};
use objc2_web_kit::{WKNavigation, WKWebView};

use super::{url_from_webview, InnerWebView, WryWebView};
use crate::PageLoadEvent;

extern "C" fn did_commit_navigation(
  this: &NSObject,
  _: Sel,
  webview: &WKWebView,
  _navigation: &WKNavigation,
) {
  unsafe {
    // Call on_load_handler
    let on_page_load = this.get_ivar::<*mut c_void>("on_page_load_function");
    if !on_page_load.is_null() {
      let on_page_load = &mut *(*on_page_load as *mut Box<dyn Fn(PageLoadEvent)>);
      on_page_load(PageLoadEvent::Started);
    }

    // Inject scripts
    let pending_scripts_ptr: *mut c_void = *this.get_ivar("pending_scripts");
    let pending_scripts = &(*(pending_scripts_ptr as *mut Arc<Mutex<Option<Vec<String>>>>));
    let mut pending_scripts_ = pending_scripts.lock().unwrap();
    if let Some(pending_scripts) = &*pending_scripts_ {
      for script in pending_scripts {
        webview.evaluateJavaScript_completionHandler(&NSString::from_str(script), None);
      }
      *pending_scripts_ = None;
    }
  }
}

extern "C" fn did_finish_navigation(
  this: &NSObject,
  _: Sel,
  _webview: &WKWebView,
  _navigation: &WKNavigation,
) {
  unsafe {
    // Call on_load_handler
    let on_page_load = this.get_ivar::<*mut c_void>("on_page_load_function");
    if !on_page_load.is_null() {
      let on_page_load = &mut *(*on_page_load as *mut Box<dyn Fn(PageLoadEvent)>);
      on_page_load(PageLoadEvent::Finished);
    }
  }
}

pub(crate) unsafe fn add_navigation_mathods(cls: &mut ClassBuilder) {
  cls.add_ivar::<*mut c_void>("navigation_policy_function");
  cls.add_ivar::<*mut c_void>("on_page_load_function");

  cls.add_method(
    objc2::sel!(webView:didFinishNavigation:),
    did_finish_navigation as extern "C" fn(_, _, _, _),
  );
  cls.add_method(
    objc2::sel!(webView:didCommitNavigation:),
    did_commit_navigation as extern "C" fn(_, _, _, _),
  );
}

pub(crate) unsafe fn drop_navigation_methods(inner: &mut InnerWebView) {
  if !inner.page_load_handler.is_null() {
    drop(Box::from_raw(inner.page_load_handler))
  }
}

pub(crate) unsafe fn set_navigation_methods(
  navigation_policy_handler: *mut AnyObject,
  webview: Retained<WryWebView>,
  on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
) -> *mut Box<dyn Fn(PageLoadEvent)> {
  if let Some(on_page_load_handler) = on_page_load_handler {
    let on_page_load_handler = Box::into_raw(Box::new(Box::new(move |event| {
      on_page_load_handler(event, url_from_webview(&webview).unwrap_or_default());
    }) as Box<dyn Fn(PageLoadEvent)>));

    let ivar = (*navigation_policy_handler)
      .class()
      .instance_variable("on_page_load_function")
      .unwrap();
    let ivar_delegate = ivar.load_mut(&mut *navigation_policy_handler);
    *ivar_delegate = on_page_load_handler as *mut _ as *mut c_void;

    on_page_load_handler
  } else {
    null_mut()
  }
}

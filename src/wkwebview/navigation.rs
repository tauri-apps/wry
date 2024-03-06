use std::{
  ffi::c_void,
  ptr::{null, null_mut},
  sync::{Arc, Mutex},
};

use cocoa::base::id;
use objc::{
  declare::ClassDecl,
  runtime::{Object, Sel},
};

use super::{url_from_webview, InnerWebView, NSString};
use crate::PageLoadEvent;

extern "C" fn did_commit_navigation(this: &Object, _: Sel, webview: id, _navigation: id) {
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
        let _: id = msg_send![webview, evaluateJavaScript:NSString::new(script) completionHandler:null::<*const c_void>()];
      }
      *pending_scripts_ = None;
    }
  }
}

extern "C" fn did_finish_navigation(this: &Object, _: Sel, _webview: id, _navigation: id) {
  unsafe {
    // Call on_load_handler
    let on_page_load = this.get_ivar::<*mut c_void>("on_page_load_function");
    if !on_page_load.is_null() {
      let on_page_load = &mut *(*on_page_load as *mut Box<dyn Fn(PageLoadEvent)>);
      on_page_load(PageLoadEvent::Finished);
    }
  }
}

pub(crate) unsafe fn add_navigation_mathods(cls: &mut ClassDecl) {
  cls.add_ivar::<*mut c_void>("navigation_policy_function");
  cls.add_ivar::<*mut c_void>("on_page_load_function");

  cls.add_method(
    sel!(webView:didFinishNavigation:),
    did_finish_navigation as extern "C" fn(&Object, Sel, id, id),
  );
  cls.add_method(
    sel!(webView:didCommitNavigation:),
    did_commit_navigation as extern "C" fn(&Object, Sel, id, id),
  );
}

pub(crate) unsafe fn drop_navigation_methods(inner: &mut InnerWebView) {
  if !inner.page_load_handler.is_null() {
    drop(Box::from_raw(inner.page_load_handler))
  }
}

pub(crate) unsafe fn set_navigation_methods(
  navigation_policy_handler: *mut Object,
  webview: id,
  on_page_load_handler: Option<Box<dyn Fn(PageLoadEvent, String)>>,
) -> *mut Box<dyn Fn(PageLoadEvent)> {
  if let Some(on_page_load_handler) = on_page_load_handler {
    let on_page_load_handler = Box::into_raw(Box::new(Box::new(move |event| {
      on_page_load_handler(event, url_from_webview(webview).unwrap_or_default());
    }) as Box<dyn Fn(PageLoadEvent)>));
    (*navigation_policy_handler).set_ivar(
      "on_page_load_function",
      on_page_load_handler as *mut _ as *mut c_void,
    );
    on_page_load_handler
  } else {
    null_mut()
  }
}

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

extern "C" fn did_commit_navigation(this: &Object, _: Sel, webview: id, _navigation: id) {
  unsafe {
    // Call on_load_handler
    let on_loading = this.get_ivar::<*mut c_void>("on_page_loading_function");
    if !on_loading.is_null() {
      let on_loading = &mut *(*on_loading as *mut Box<dyn Fn()>);
      on_loading();
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
    let on_loaded = this.get_ivar::<*mut c_void>("on_page_loaded_function");
    if !on_loaded.is_null() {
      let on_loaded = &mut *(*on_loaded as *mut Box<dyn Fn()>);
      on_loaded();
    }
  }
}

pub(crate) unsafe fn add_navigation_mathods(cls: &mut ClassDecl) {
  cls.add_ivar::<*mut c_void>("navigation_policy_function");
  cls.add_ivar::<*mut c_void>("on_page_loading_function");
  cls.add_ivar::<*mut c_void>("on_page_loaded_function");

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
  if !inner.loaded_handler.is_null() {
    drop(Box::from_raw(inner.loaded_handler))
  }
  if !inner.loading_handler.is_null() {
    drop(Box::from_raw(inner.loading_handler))
  }
}

pub(crate) unsafe fn set_navigation_methods(
  navigation_policy_handler: *mut Object,
  webview: id,
  on_page_loading_handler: Option<Box<dyn Fn(String)>>,
  on_page_loaded_handler: Option<Box<dyn Fn(String)>>,
) -> (*mut Box<dyn Fn()>, *mut Box<dyn Fn()>) {
  let loading_handler = if let Some(on_page_loading_handler) = on_page_loading_handler {
    let on_page_loading_handler = Box::into_raw(Box::new(Box::new(move || {
      on_page_loading_handler(url_from_webview(webview));
    }) as Box<dyn Fn()>));
    (*navigation_policy_handler).set_ivar(
      "on_page_loading_function",
      on_page_loading_handler as *mut _ as *mut c_void,
    );
    on_page_loading_handler
  } else {
    null_mut()
  };

  let loaded_handler = if let Some(on_page_loaded_handler) = on_page_loaded_handler {
    let on_page_loaded_handler = Box::into_raw(Box::new(Box::new(move || {
      on_page_loaded_handler(url_from_webview(webview));
    }) as Box<dyn Fn()>));
    (*navigation_policy_handler).set_ivar(
      "on_page_loaded_function",
      on_page_loaded_handler as *mut _ as *mut c_void,
    );
    on_page_loaded_handler
  } else {
    null_mut()
  };
  (loading_handler, loaded_handler)
}

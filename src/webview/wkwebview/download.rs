use cocoa::base::id;
use libc::c_void;
use objc::{runtime::{Object, Sel}, declare::ClassDecl};

pub(crate) unsafe fn set_download_delegate(
  webview: *mut Object,
  download_delegate: *mut Object
) {
  (*webview).set_ivar("DownloadDelegate", download_delegate as *mut _ as *mut c_void);
}

unsafe fn get_download_delegate(this: &mut Object) -> *mut objc::runtime::Object {
  let delegate: *mut c_void = *this.get_ivar("DownloadDelegate");
  delegate as *mut Object
}

// Download action handler
extern "C" fn navigation_download_action(this: &mut Object, _: Sel, _: id, _: id, download: id) {
  unsafe {
    let delegate = get_download_delegate(this);
    let _: () = msg_send![download, setDelegate: delegate];
  }
}

// Download response handler
extern "C" fn navigation_download_response(this: &mut Object, _: Sel, _: id, _: id, download: id) {
  unsafe {
    let delegate = get_download_delegate(this);
    let _: () = msg_send![download, setDelegate: delegate];
  }
}

pub(crate) unsafe fn add_download_methods(decl: &mut ClassDecl) {
  decl.add_ivar::<*mut c_void>("DownloadDelegate");

  decl.add_method(
    sel!(webView:navigationAction:didBecomeDownload:),
    navigation_download_action as extern "C" fn(&mut Object, Sel, id, id, id),
  );

  decl.add_method(
    sel!(webView:navigationResponse:didBecomeDownload:),
    navigation_download_response as extern "C" fn(&mut Object, Sel, id, id, id),
  );
}
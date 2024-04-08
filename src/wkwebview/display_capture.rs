use crate::operating_system_version;
use cocoa::base::id;
use objc::{
  declare::ClassDecl,
  runtime::{Object, Sel},
};
use std::ffi::c_void;

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKMediaCaptureType {
  Camera = 0,
  Microphone,
  CameraAndMicrophone,
}

impl From<isize> for WKMediaCaptureType {
  fn from(value: isize) -> Self {
    match value {
      0 => WKMediaCaptureType::Camera,
      1 => WKMediaCaptureType::Microphone,
      2 => WKMediaCaptureType::CameraAndMicrophone,
      _ => panic!("Invalid WKMediaCaptureType value"),
    }
  }
}

#[repr(isize)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WKDisplayCapturePermissionDecision {
  Deny = 0,
  ScreenPrompt,
  WindowPrompt,
}

impl From<isize> for WKDisplayCapturePermissionDecision {
  fn from(value: isize) -> Self {
    match value {
      0 => WKDisplayCapturePermissionDecision::Deny,
      1 => WKDisplayCapturePermissionDecision::ScreenPrompt,
      2 => WKDisplayCapturePermissionDecision::WindowPrompt,
      _ => panic!("Invalid WKDisplayCapturePermissionDecision value"),
    }
  }
}

pub(crate) fn declare_decision_handler(ctl: &mut ClassDecl) {
  #[cfg(target_os = "macos")]
  if operating_system_version().0 >= 13 {
    unsafe {
      ctl.add_ivar::<*mut c_void>("display_capture_decision_handler");
      ctl.add_method(sel!(_webView:requestDisplayCapturePermissionForOrigin:initiatedByFrame:withSystemAudio:decisionHandler:),
        request_display_capture_permission as extern "C" fn(&Object, Sel, id, id, id, isize, id),);
    }
  }
}

pub(crate) fn set_decision_handler(
  webview: id,
  handler: Option<Box<dyn Fn(WKMediaCaptureType) -> WKDisplayCapturePermissionDecision + 'static>>,
) {
  #[cfg(target_os = "macos")]
  if operating_system_version().0 >= 13 {
    unsafe {
      if let Some(handler) = handler {
        drop_decision_hanlder(webview);

        let ui_delegate: id = msg_send![webview, UIDelegate];
        let handler = Box::into_raw(Box::new(handler));
        (*ui_delegate).set_ivar(
          "display_capture_decision_handler",
          handler as *mut _ as *mut c_void,
        );
      }
    }
  }
}

pub(crate) fn drop_decision_hanlder(webview: *mut Object) {
  unsafe {
    let ui_delegate: id = msg_send![webview, UIDelegate];
    let function = (*ui_delegate).get_ivar::<*mut c_void>("display_capture_decision_handler");
    if !function.is_null() {
      let function = *function
        as *mut Box<dyn for<'s> Fn(WKMediaCaptureType) -> WKDisplayCapturePermissionDecision>;
      drop(Box::from_raw(function));
    }
  }
}

extern "C" fn request_display_capture_permission(
  this: &Object,
  _: Sel,
  _webview: id,
  _origin: id,
  _frame: id,
  capture_type: isize,
  decision_handler: id,
) {
  unsafe {
    let decision_handler =
      decision_handler as *mut block::Block<(WKDisplayCapturePermissionDecision,), c_void>;

    let function = this.get_ivar::<*mut c_void>("display_capture_decision_handler");
    if !function.is_null() {
      let function = *function
        as *mut Box<dyn for<'s> Fn(WKMediaCaptureType) -> WKDisplayCapturePermissionDecision>;

      let decision = (*function)(WKMediaCaptureType::from(capture_type));
      (*decision_handler).call((decision,));
    } else {
      (*decision_handler).call((WKDisplayCapturePermissionDecision::Deny,));
    }
  }
}

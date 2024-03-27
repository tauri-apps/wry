// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  ffi::{c_void, CStr},
  path::PathBuf,
};

use icrate::{
  AppKit::{NSDragOperation, NSDraggingInfo, NSFilenamesPboardType, NSView},
  Foundation::{NSArray, NSPoint, NSRect, NSString},
};
use objc2::{
  class,
  declare::ClassBuilder,
  rc::Id,
  runtime::{AnyObject, Bool, ProtocolObject, Sel},
};
use once_cell::sync::Lazy;

use crate::DragDropEvent;

// pub(crate) type NSDragOperation = cocoa::foundation::NSUInteger;

#[allow(non_upper_case_globals)]
const NSDragOperationCopy: NSDragOperation = 1;

const DRAG_DROP_HANDLER_IVAR: &str = "DragDropHandler";

static OBJC_DRAGGING_ENTERED: Lazy<
  extern "C" fn(&NSView, Sel, &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation,
> = Lazy::new(|| unsafe {
  std::mem::transmute(
    class!(WKWebView)
      .instance_method(objc2::sel!(draggingEntered:))
      .unwrap()
      .implementation(),
  )
});

static OBJC_DRAGGING_EXITED: Lazy<
  extern "C" fn(&NSView, Sel, &ProtocolObject<dyn NSDraggingInfo>),
> = Lazy::new(|| unsafe {
  std::mem::transmute(
    class!(WKWebView)
      .instance_method(objc2::sel!(draggingExited:))
      .unwrap()
      .implementation(),
  )
});

static OBJC_PERFORM_DRAG_OPERATION: Lazy<
  extern "C" fn(&NSView, Sel, &ProtocolObject<dyn NSDraggingInfo>) -> objc2::runtime::Bool,
> = Lazy::new(|| unsafe {
  std::mem::transmute(
    class!(WKWebView)
      .instance_method(objc2::sel!(performDragOperation:))
      .unwrap()
      .implementation(),
  )
});

static OBJC_DRAGGING_UPDATED: Lazy<
  extern "C" fn(&NSView, Sel, &ProtocolObject<dyn NSDraggingInfo>) -> NSDragOperation,
> = Lazy::new(|| unsafe {
  std::mem::transmute(
    class!(WKWebView)
      .instance_method(objc2::sel!(draggingUpdated:))
      .unwrap()
      .implementation(),
  )
});

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn set_drag_drop_handler(
  webview: *mut AnyObject,
  handler: Box<dyn Fn(DragDropEvent) -> bool>,
) -> *mut Box<dyn Fn(DragDropEvent) -> bool> {
  let listener = Box::into_raw(Box::new(handler));
  let ivar = (*webview)
    .class()
    .instance_variable(DRAG_DROP_HANDLER_IVAR)
    .unwrap();
  let mut r = ivar.load_mut::<*mut c_void>(webview.as_mut().unwrap());
  r.replace(*(listener as *mut c_void));
  // (*webview).set_ivar(DRAG_DROP_HANDLER_IVAR, listener as *mut _ as *mut c_void);
  listener
}

#[allow(clippy::mut_from_ref)]
unsafe fn get_handler(this: &AnyObject) -> &mut Box<dyn Fn(DragDropEvent) -> bool> {
  let delegate: *mut c_void = *this.get_ivar(DRAG_DROP_HANDLER_IVAR);
  &mut *(delegate as *mut Box<dyn Fn(DragDropEvent) -> bool>)
}

unsafe fn collect_paths(drag_info: &ProtocolObject<dyn NSDraggingInfo>) -> Vec<PathBuf> {
  let pb = drag_info.draggingPasteboard();
  let mut drag_drop_paths = Vec::new();
  let types = NSArray::arrayWithObject(NSFilenamesPboardType);
  if pb.availableTypeFromArray(&types).is_some() {
    for path in pb.propertyListForType(NSFilenamesPboardType).iter() {
      let path = Id::<AnyObject>::cast(*path) as Id<NSString>;
      drag_drop_paths.push(PathBuf::from(
        CStr::from_ptr(NSString::UTF8String(&path))
          .to_string_lossy()
          .into_owned(),
      ));
    }
  }
  drag_drop_paths
}

extern "C" fn dragging_updated(
  this: &NSView,
  sel: Sel,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> NSDragOperation {
  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = unsafe { this.frame() };
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);
  let listener = unsafe { get_handler(this) };
  if !listener(DragDropEvent::Over { position }) {
    let os_operation = OBJC_DRAGGING_UPDATED(this, sel, drag_info);
    if os_operation == 0 {
      // 0 will be returned for a drop on any arbitrary location on the webview.
      // We'll override that with NSDragOperationCopy.
      NSDragOperationCopy
    } else {
      // A different NSDragOperation is returned when a file is hovered over something like
      // a <input type="file">, so we'll make sure to preserve that behaviour.
      os_operation
    }
  } else {
    NSDragOperationCopy
  }
}

extern "C" fn dragging_entered(
  this: &NSView,
  sel: Sel,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> NSDragOperation {
  let listener = unsafe { get_handler(this) };
  let paths = unsafe { collect_paths(drag_info) };

  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = unsafe { this.frame() };
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  if !listener(DragDropEvent::Enter { paths, position }) {
    // Reject the Wry file drop (invoke the OS default behaviour)
    OBJC_DRAGGING_ENTERED(this, sel, drag_info)
  } else {
    NSDragOperationCopy
  }
}

extern "C" fn perform_drag_operation(
  this: &NSView,
  sel: Sel,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> Bool {
  let listener = unsafe { get_handler(this) };
  let paths = unsafe { collect_paths(drag_info) };

  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = unsafe { this.frame() };
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  if !listener(DragDropEvent::Drop { paths, position }) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_PERFORM_DRAG_OPERATION(this, sel, drag_info)
  } else {
    Bool::YES
  }
}

extern "C" fn dragging_exited(
  this: &NSView,
  sel: Sel,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) {
  let listener = unsafe { get_handler(this) };
  if !listener(DragDropEvent::Leave) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_DRAGGING_EXITED(this, sel, drag_info);
  }
}

pub(crate) unsafe fn add_drag_drop_methods(decl: &mut ClassBuilder) {
  decl.add_ivar::<*mut c_void>(DRAG_DROP_HANDLER_IVAR);

  decl.add_method(
    objc2::sel!(draggingEntered:),
    dragging_entered as extern "C" fn(_, _, _) -> NSDragOperation,
  );

  decl.add_method(
    objc2::sel!(draggingUpdated:),
    dragging_updated as extern "C" fn(_, _, _) -> NSDragOperation,
  );

  decl.add_method(
    objc2::sel!(performDragOperation:),
    perform_drag_operation as extern "C" fn(_, _, _) -> Bool,
  );

  decl.add_method(
    objc2::sel!(draggingExited:),
    dragging_exited as extern "C" fn(_, _, _),
  );
}

// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  ffi::{c_void, CStr},
  path::PathBuf,
};

use cocoa::{
  base::{id, BOOL, YES},
  foundation::{NSPoint, NSRect},
};
use objc::{
  declare::ClassDecl,
  runtime::{class_getInstanceMethod, method_getImplementation, Object, Sel},
};
use once_cell::sync::Lazy;

use crate::DragDropEvent;

pub(crate) type NSDragOperation = cocoa::foundation::NSUInteger;

#[allow(non_upper_case_globals)]
const NSDragOperationCopy: NSDragOperation = 1;

const DRAG_DROP_HANDLER_IVAR: &str = "DragDropHandler";

static OBJC_DRAGGING_ENTERED: Lazy<extern "C" fn(*const Object, Sel, id) -> NSDragOperation> =
  Lazy::new(|| unsafe {
    std::mem::transmute(method_getImplementation(class_getInstanceMethod(
      class!(WKWebView),
      sel!(draggingEntered:),
    )))
  });

static OBJC_DRAGGING_EXITED: Lazy<extern "C" fn(*const Object, Sel, id)> = Lazy::new(|| unsafe {
  std::mem::transmute(method_getImplementation(class_getInstanceMethod(
    class!(WKWebView),
    sel!(draggingExited:),
  )))
});

static OBJC_PERFORM_DRAG_OPERATION: Lazy<extern "C" fn(*const Object, Sel, id) -> BOOL> =
  Lazy::new(|| unsafe {
    std::mem::transmute(method_getImplementation(class_getInstanceMethod(
      class!(WKWebView),
      sel!(performDragOperation:),
    )))
  });

static OBJC_DRAGGING_UPDATED: Lazy<extern "C" fn(*const Object, Sel, id) -> NSDragOperation> =
  Lazy::new(|| unsafe {
    std::mem::transmute(method_getImplementation(class_getInstanceMethod(
      class!(WKWebView),
      sel!(draggingUpdated:),
    )))
  });

// Safety: objc runtime calls are unsafe
pub(crate) unsafe fn set_drag_drop_handler(
  webview: *mut Object,
  handler: Box<dyn Fn(DragDropEvent) -> bool>,
) -> *mut Box<dyn Fn(DragDropEvent) -> bool> {
  let listener = Box::into_raw(Box::new(handler));
  (*webview).set_ivar(DRAG_DROP_HANDLER_IVAR, listener as *mut _ as *mut c_void);
  listener
}

#[allow(clippy::mut_from_ref)]
unsafe fn get_handler(this: &Object) -> &mut Box<dyn Fn(DragDropEvent) -> bool> {
  let delegate: *mut c_void = *this.get_ivar(DRAG_DROP_HANDLER_IVAR);
  &mut *(delegate as *mut Box<dyn Fn(DragDropEvent) -> bool>)
}

unsafe fn collect_paths(drag_info: id) -> Vec<PathBuf> {
  use cocoa::{
    appkit::{NSFilenamesPboardType, NSPasteboard},
    foundation::{NSFastEnumeration, NSString},
  };

  let pb: id = msg_send![drag_info, draggingPasteboard];
  let mut drag_drop_paths = Vec::new();
  let types: id = msg_send![class!(NSArray), arrayWithObject: NSFilenamesPboardType];
  if !NSPasteboard::availableTypeFromArray(pb, types).is_null() {
    for path in NSPasteboard::propertyListForType(pb, NSFilenamesPboardType).iter() {
      drag_drop_paths.push(PathBuf::from(
        CStr::from_ptr(NSString::UTF8String(path))
          .to_string_lossy()
          .into_owned(),
      ));
    }
  }
  drag_drop_paths
}

extern "C" fn dragging_updated(this: &mut Object, sel: Sel, drag_info: id) -> NSDragOperation {
  let dl: NSPoint = unsafe { msg_send![drag_info, draggingLocation] };
  let frame: NSRect = unsafe { msg_send![this, frame] };
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

extern "C" fn dragging_entered(this: &mut Object, sel: Sel, drag_info: id) -> NSDragOperation {
  let listener = unsafe { get_handler(this) };
  let paths = unsafe { collect_paths(drag_info) };

  let dl: NSPoint = unsafe { msg_send![drag_info, draggingLocation] };
  let frame: NSRect = unsafe { msg_send![this, frame] };
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  if !listener(DragDropEvent::Enter { paths, position }) {
    // Reject the Wry file drop (invoke the OS default behaviour)
    OBJC_DRAGGING_ENTERED(this, sel, drag_info)
  } else {
    NSDragOperationCopy
  }
}

extern "C" fn perform_drag_operation(this: &mut Object, sel: Sel, drag_info: id) -> BOOL {
  let listener = unsafe { get_handler(this) };
  let paths = unsafe { collect_paths(drag_info) };

  let dl: NSPoint = unsafe { msg_send![drag_info, draggingLocation] };
  let frame: NSRect = unsafe { msg_send![this, frame] };
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  if !listener(DragDropEvent::Drop { paths, position }) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_PERFORM_DRAG_OPERATION(this, sel, drag_info)
  } else {
    YES
  }
}

extern "C" fn dragging_exited(this: &mut Object, sel: Sel, drag_info: id) {
  let listener = unsafe { get_handler(this) };
  if !listener(DragDropEvent::Leave) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_DRAGGING_EXITED(this, sel, drag_info);
  }
}

pub(crate) unsafe fn add_drag_drop_methods(decl: &mut ClassDecl) {
  decl.add_ivar::<*mut c_void>(DRAG_DROP_HANDLER_IVAR);

  decl.add_method(
    sel!(draggingEntered:),
    dragging_entered as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
  );

  decl.add_method(
    sel!(draggingUpdated:),
    dragging_updated as extern "C" fn(&mut Object, Sel, id) -> NSDragOperation,
  );

  decl.add_method(
    sel!(performDragOperation:),
    perform_drag_operation as extern "C" fn(&mut Object, Sel, id) -> BOOL,
  );

  decl.add_method(
    sel!(draggingExited:),
    dragging_exited as extern "C" fn(&mut Object, Sel, id),
  );
}

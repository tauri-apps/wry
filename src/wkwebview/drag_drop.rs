// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{ffi::CStr, path::PathBuf};

use objc2::{
  class,
  rc::Id,
  runtime::{AnyObject, Bool, ProtocolObject, Sel},
  DeclaredClass,
};
use objc2_app_kit::{NSDragOperation, NSDraggingInfo, NSFilenamesPboardType, NSView};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSString};
use once_cell::sync::Lazy;

use crate::DragDropEvent;

use super::WryWebView;

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

pub(crate) unsafe fn collect_paths(drag_info: &ProtocolObject<dyn NSDraggingInfo>) -> Vec<PathBuf> {
  let pb = drag_info.draggingPasteboard();
  let mut drag_drop_paths = Vec::new();
  let types = NSArray::arrayWithObject(NSFilenamesPboardType);

  if pb.availableTypeFromArray(&types).is_some() {
    let paths = pb.propertyListForType(NSFilenamesPboardType).unwrap();
    let paths: Id<NSArray<NSString>> = Id::<AnyObject>::cast(paths.clone());
    for path in paths.to_vec() {
      let path = CStr::from_ptr(path.UTF8String()).to_string_lossy();
      drag_drop_paths.push(PathBuf::from(path.into_owned()));
    }
  }
  drag_drop_paths
}

pub(crate) fn dragging_entered(
  this: &WryWebView,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> NSDragOperation {
  let paths = unsafe { collect_paths(drag_info) };
  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = this.frame();
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  let listener = &this.ivars().drag_drop_handler;
  if !listener(DragDropEvent::Enter { paths, position }) {
    // Reject the Wry file drop (invoke the OS default behaviour)
    OBJC_DRAGGING_ENTERED(this, objc2::sel!(draggingEntered:), drag_info)
  } else {
    NSDragOperation::Copy
  }
}

pub(crate) fn dragging_updated(
  this: &WryWebView,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> NSDragOperation {
  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = this.frame();
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  let listener = &this.ivars().drag_drop_handler;
  if !listener(DragDropEvent::Over { position }) {
    let os_operation = OBJC_DRAGGING_UPDATED(this, objc2::sel!(draggingUpdated:), drag_info);
    if os_operation == NSDragOperation::None {
      // 0 will be returned for a drop on any arbitrary location on the webview.
      // We'll override that with NSDragOperationCopy.
      NSDragOperation::Copy
    } else {
      // A different NSDragOperation is returned when a file is hovered over something like
      // a <input type="file">, so we'll make sure to preserve that behaviour.
      os_operation
    }
  } else {
    NSDragOperation::Copy
  }
}

pub(crate) fn perform_drag_operation(
  this: &WryWebView,
  drag_info: &ProtocolObject<dyn NSDraggingInfo>,
) -> Bool {
  let paths = unsafe { collect_paths(drag_info) };
  let dl: NSPoint = unsafe { drag_info.draggingLocation() };
  let frame: NSRect = this.frame();
  let position = (dl.x as i32, (frame.size.height - dl.y) as i32);

  let listener = &this.ivars().drag_drop_handler;
  if !listener(DragDropEvent::Drop { paths, position }) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_PERFORM_DRAG_OPERATION(this, objc2::sel!(performDragOperation:), drag_info)
  } else {
    Bool::YES
  }
}

pub(crate) fn dragging_exited(this: &WryWebView, drag_info: &ProtocolObject<dyn NSDraggingInfo>) {
  let listener = &this.ivars().drag_drop_handler;
  if !listener(DragDropEvent::Leave) {
    // Reject the Wry drop (invoke the OS default behaviour)
    OBJC_DRAGGING_EXITED(this, objc2::sel!(draggingExited:), drag_info);
  }
}

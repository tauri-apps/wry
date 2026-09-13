// Copyright 2020-2023 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{ffi::CStr, path::PathBuf};

use objc2::{
  runtime::{Bool, ProtocolObject},
  ClassType, DeclaredClass,
};
use objc2_app_kit::{NSDragOperation, NSDraggingInfo, NSFilenamesPboardType};
use objc2_foundation::{NSArray, NSPoint, NSRect, NSString, NSURL};

use crate::DragDropEvent;

use super::WryWebView;

pub(crate) unsafe fn collect_paths(drag_info: &ProtocolObject<dyn NSDraggingInfo>) -> Vec<PathBuf> {
  // Prefer the modern `readObjectsForClasses:options:` API with `NSURL`. It works
  // for sources that publish per-item `public.file-url` types (the standard for
  // `NSDraggingItem.initWithPasteboardWriter:` with `NSPasteboardItem`) as well
  // as the legacy `NSFilenamesPboardType` payload, so it covers both paths in
  // one call.
  //
  // The legacy `NSFilenamesPboardType` branch below is kept as a defensive
  // fallback in case `readObjectsForClasses:options:` returns nothing on some
  // configuration; it should not normally be reached.
  //
  // No `unwrap` calls: every conversion that can fail skips the entry, so a
  // malformed pasteboard never panics the webview process.
  let pb = drag_info.draggingPasteboard();
  let mut drag_drop_paths = Vec::new();

  let url_classes = NSArray::from_slice(&[NSURL::class()]);
  if let Some(items) = pb.readObjectsForClasses_options(&url_classes, None) {
    for item in &items {
      if let Some(url) = item.downcast_ref::<NSURL>() {
        if let Some(path) = url.path() {
          let path = CStr::from_ptr(path.UTF8String()).to_string_lossy();
          drag_drop_paths.push(PathBuf::from(path.into_owned()));
        }
      }
    }
    if !drag_drop_paths.is_empty() {
      return drag_drop_paths;
    }
  }

  // Legacy fallback: read `NSFilenamesPboardType` directly.
  let types = NSArray::arrayWithObject(NSFilenamesPboardType);
  if pb.availableTypeFromArray(&types).is_some() {
    if let Some(paths) = pb.propertyListForType(NSFilenamesPboardType) {
      if let Ok(paths) = paths.downcast::<NSArray>() {
        for path in paths {
          if let Ok(path) = path.downcast::<NSString>() {
            let path = CStr::from_ptr(path.UTF8String()).to_string_lossy();
            drag_drop_paths.push(PathBuf::from(path.into_owned()));
          }
        }
      }
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
    unsafe { objc2::msg_send![super(this), draggingEntered: drag_info] }
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
    unsafe {
      let os_operation = objc2::msg_send![super(this), draggingUpdated: drag_info];
      if os_operation == NSDragOperation::None {
        // 0 will be returned for a drop on any arbitrary location on the webview.
        // We'll override that with NSDragOperationCopy.
        NSDragOperation::Copy
      } else {
        // A different NSDragOperation is returned when a file is hovered over something like
        // a <input type="file">, so we'll make sure to preserve that behaviour.
        os_operation
      }
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
    unsafe { objc2::msg_send![super(this), performDragOperation: drag_info] }
  } else {
    Bool::YES
  }
}

pub(crate) fn dragging_exited(this: &WryWebView, drag_info: &ProtocolObject<dyn NSDraggingInfo>) {
  let listener = &this.ivars().drag_drop_handler;
  if !listener(DragDropEvent::Leave) {
    // Reject the Wry drop (invoke the OS default behaviour)
    unsafe { objc2::msg_send![super(this), draggingExited: drag_info] }
  }
}

// Copyright 2020-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use objc2::{
  declare_class,
  mutability::MainThreadOnly,
  rc::Retained,
  runtime::{Bool, MessageReceiver, ProtocolObject},
  ClassType, DeclaredClass,
};
use objc2_app_kit::{NSDraggingDestination, NSEvent};
use objc2_foundation::{NSObjectProtocol, NSUUID};
use objc2_web_kit::WKWebView;

use crate::{
  wkwebview::{drag_drop, synthetic_mouse_events},
  DragDropEvent,
};

pub struct WryWebViewIvars {
  pub(crate) is_child: bool,
  #[cfg(target_os = "macos")]
  pub(crate) drag_drop_handler: Box<dyn Fn(DragDropEvent) -> bool>,
  #[cfg(target_os = "macos")]
  pub(crate) accept_first_mouse: objc2::runtime::Bool,
  pub(crate) custom_protocol_task_ids: HashMap<usize, Retained<NSUUID>>,
}

declare_class!(
  pub struct WryWebView;

  unsafe impl ClassType for WryWebView {
    type Super = WKWebView;
    type Mutability = MainThreadOnly;
    const NAME: &'static str = "WryWebView";
  }

  impl DeclaredClass for WryWebView {
    type Ivars = WryWebViewIvars;
  }

  unsafe impl WryWebView {
    #[method(performKeyEquivalent:)]
    fn perform_key_equivalent(
      &self,
      _event: &NSEvent,
    ) -> Bool {
      // This is a temporary workaround for https://github.com/tauri-apps/tauri/issues/9426
      // FIXME: When the webview is a child webview, performKeyEquivalent always return YES
      // and stop propagating the event to the window, hence the menu shortcut won't be
      // triggered. However, overriding this method also means the cmd+key event won't be
      // handled in webview, which means the key cannot be listened by JavaScript.
      if self.ivars().is_child {
        Bool::NO
      } else {
        unsafe {
          let result: Bool = self.send_super_message(
            WKWebView::class(),
            objc2::sel!(performKeyEquivalent:),
            (_event,),
          );
          result
        }
      }
    }

    #[method(acceptsFirstMouse:)]
    fn accept_first_mouse(
      &self,
      _event: &NSEvent,
    ) -> Bool {
        self.ivars().accept_first_mouse
    }
  }
  unsafe impl NSObjectProtocol for WryWebView {}

  // Drag & Drop
  #[cfg(target_os = "macos")]
  unsafe impl NSDraggingDestination for WryWebView {
    #[method(draggingEntered:)]
    fn dragging_entered(
      &self,
      drag_info: &ProtocolObject<dyn objc2_app_kit::NSDraggingInfo>,
    ) -> objc2_app_kit::NSDragOperation {
      drag_drop::dragging_entered(self, drag_info)
    }

    #[method(draggingUpdated:)]
    fn dragging_updated(
      &self,
      drag_info: &ProtocolObject<dyn objc2_app_kit::NSDraggingInfo>,
    ) -> objc2_app_kit::NSDragOperation {
      drag_drop::dragging_updated(self, drag_info)
    }

    #[method(performDragOperation:)]
    fn perform_drag_operation(
      &self,
      drag_info: &ProtocolObject<dyn objc2_app_kit::NSDraggingInfo>,
    ) -> Bool {
      drag_drop::perform_drag_operation(self, drag_info)
    }

    #[method(draggingExited:)]
    fn dragging_exited(
      &self,
      drag_info: &ProtocolObject<dyn objc2_app_kit::NSDraggingInfo>,
    ) {
      drag_drop::dragging_exited(self, drag_info)
    }
  }

  // Synthetic mouse events
  #[cfg(target_os = "macos")]
  unsafe impl WryWebView {
    #[method(otherMouseDown:)]
    fn other_mouse_down(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_down(self, event)
    }

    #[method(otherMouseUp:)]
    fn other_mouse_up(
      &self,
      event: &NSEvent,
    ) {
      synthetic_mouse_events::other_mouse_up(self, event)
    }
  }
);

// Custom Protocol Task Checker
impl WryWebView {
  pub(crate) fn add_custom_task_key(&mut self, task_id: usize) -> Retained<NSUUID> {
    let task_uuid = NSUUID::new();
    self
      .ivars_mut()
      .custom_protocol_task_ids
      .insert(task_id, task_uuid.clone());
    task_uuid
  }
  pub(crate) fn remove_custom_task_key(&mut self, task_id: usize) {
    self.ivars_mut().custom_protocol_task_ids.remove(&task_id);
  }
  pub(crate) fn get_custom_task_uuid(&self, task_id: usize) -> Option<Retained<NSUUID>> {
    self.ivars().custom_protocol_task_ids.get(&task_id).cloned()
  }
}
